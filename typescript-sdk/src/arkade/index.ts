/**
 * Arkade Intents venue — `@kaleidorg/swap-sdk/arkade`.
 *
 * Wraps `@arkade-os/swap`'s RFQ client behind the SDK's venue vocabulary:
 * prepare (quote + derive + persist), fund (the caller's wallet), and a
 * resumable `reconcile()` the host drives from its own scheduler (MV3
 * `chrome.alarms`, a node interval — the venue never owns a timer).
 *
 * ## Why a subpath
 *
 * Same reasoning as `@arkade-os/swap/nostr`: a Boltz-only consumer of
 * `@kaleidorg/swap-sdk` should not pay for the Arkade dependency graph.
 * `@arkade-os/swap` and `@arkade-os/sdk` are optional peer dependencies,
 * resolved only when this subpath is imported — MV3 service workers forbid
 * dynamic `import()`, so the imports below are static and the module is
 * opt-in at the bundler level instead.
 *
 * ## The persistence contract
 *
 * Every `prepare*` call writes its recovery record to the injected
 * {@link ArkadeSwapStore} BEFORE returning the funding instruction, mirroring
 * the Intents docs' rule: persist first, then move value. A store failure
 * throws while nothing is at stake.
 *
 * ## Who drives the state machine
 *
 * `@arkade-os/swap` 0.0.4+ ships `RfqSwapManager`, which owns the
 * phase/reconcile state machine this venue used to hand-roll (chain polling,
 * claim/refund dispatch, terminal-state bookkeeping). This venue keeps that
 * manager PRIVATE and drives it itself rather than exposing `start`/`stop`:
 * the manager's own timer is a `setTimeout` loop, which does not survive an
 * MV3 service worker being killed, so this venue only ever calls the
 * manager's `addSwap()`/`poll()` — never `start()`/`stop()` — and the host's
 * own scheduler (the same one that used to call `reconcile()` directly)
 * keeps being the only timer in the system. See {@link ArkadeIntentsVenue.reconcile}.
 *
 * ## Version coupling
 *
 * `@arkade-os/swap` hard-pins its own `@arkade-os/sdk`; the `wallet` object
 * crossing this boundary must come from that same SDK line. The peer ranges
 * encode it: `>=0.4.74 <0.5.0` for the SDK and `^0.0.20` for `@arkade-os/swap`
 * — both pre-1.0, so the caret pins the exact minor/patch line this module
 * was written against. The host app owns the pins.
 */

import type { IWallet } from "@arkade-os/sdk";
import { corridorRootFromMakerUrl } from "../corridor-url.js";
export { corridorRootFromMakerUrl };
import {
  ArkAddress,
  RestArkProvider,
  RestIndexerProvider,
  Transaction,
  VHTLC,
  asset,
  contractSigner,
} from "@arkade-os/sdk";
import type {
  AssetSwap,
  AssetSwapRepository,
  AvailableRfqSwapManagerCallbacks,
  InvoiceFacts,
  LightningReceiveSwap,
  LightningSendSwap,
  LockupFate,
  LockupSpendIndexer,
  LockupVtxo,
  RefundArkProvider,
  RfqQuote,
  RfqSwap,
  RfqSwapState,
  RfqTransport,
  SpendKind,
  SwapSecretsProjection,
} from "@arkade-os/swap";
import {
  LockupNeedsRecoveryError,
  PreimageNotRecoverableError,
  RfqSwapManager,
  httpTransport,
  addAssetSwap,
  cancelOffer,
  classifyDepositSpend,
  createOffer,
  decodeOffer,
  findLockupVtxos,
  getAssetSwaps,
  preimageForSwapRecord,
  pushClaim,
  pushRefundWithoutReceiver,
  readLockupFate,
  requestLightningReceive,
  requestLightningSend,
  senderIdentityForSwapRecord,
  spendTxidsOf,
  swapSecretsToRecord,
  updateAssetSwap,
} from "@arkade-os/swap";

/** Routes this venue serves today. Corridor grammar matches the Intents docs. */
export type ArkadeRoute =
  "arkade:BTC->lightning:BTC" | "lightning:BTC->arkade:BTC";

/**
 * Venue-level swap phases — the unified vocabulary hosts render from.
 *
 * `prepared` — quoted and derived; nothing funded yet.
 * `funded` — the first enforceable commitment exists (send: lockup funded;
 *            receive: hold invoice reported paid by the caller) and the
 *            underlying `RfqSwapManager` is driving it.
 * `settled` — evidence-terminal success (send: lockup claimed with the
 *            preimage; receive: we claimed the solver's lockup).
 * `refunded` — the commitment came back (timeout refund, or solver refund).
 * `cancelled` — nothing ever became enforceable (quote or invoice expired
 *            unfunded, or the funded lockup was empty the whole time it was
 *            observable).
 * `needs_recovery` — funds exist but the lockup's batch was swept; the
 *            wallet's VTXO recovery must run before a refund/claim can be
 *            pushed. Informational only within one process's lifetime — the
 *            manager keeps retrying automatically — but see
 *            {@link ArkadeSwapStore.listPending}'s contract: a record left in
 *            this phase is NOT re-attached to a fresh manager after a
 *            restart, matching this phase's original (pre-manager) contract.
 * `failed` — NEW: an action (refund push, claim push) kept failing until its
 *            window closed. Has no pre-manager equivalent; see
 *            {@link ArkadeSwapRecord.failureReason}.
 */
export type ArkadeSwapPhase =
  | "prepared"
  | "funded"
  | "settled"
  | "refunded"
  | "cancelled"
  | "needs_recovery"
  | "failed";

/** JSON-safe form of the SDK's `RelativeTimelock` (`bigint` → decimal string). */
export interface SerializedRelativeTimelock {
  type: "blocks" | "seconds";
  value: string;
}

/**
 * JSON-safe form of `VHTLC.Options.nonInteractiveParameters` — the emulator
 * covenant suite, all-or-nothing.
 */
export interface SerializedNonInteractiveParameters {
  emulatorPubkeyHex: string;
  receiverPkScriptHex: string;
  senderPkScriptHex: string;
  /**
   * `"preTimelockedRefund"` when this covenant was — or must be rebuilt as —
   * the six-plus-two-leaf shape every lockup funded before the
   * `nonInteractiveRefundWithoutReceiver` leaf shipped permanently carries.
   * `"current"` for the full nine-leaf suite.
   *
   * ALWAYS written explicitly by {@link serializeVhtlcOptions}. That is what
   * lets {@link deserializeVhtlcOptions} tell "no covenant suite" apart from
   * "a covenant suite serialized before this field existed": the latter can
   * only be the legacy shape, since it necessarily predates the leaf this
   * field distinguishes. See that function for the decode rule.
   */
  legacy?: "preTimelockedRefund" | "current";
}

/**
 * JSON-safe form of `VHTLC.Options`, complete enough that
 * `new VHTLC.ScriptV2(...)` rebuilds the identical covenant after a restart.
 */
export interface SerializedVhtlcOptions {
  senderHex: string;
  receiverHex: string;
  serverHex: string;
  preimageHashHex: string;
  /** Absolute locktime, unix seconds, as a decimal string (source is bigint). */
  refundLocktime: string;
  unilateralClaimDelay: SerializedRelativeTimelock;
  unilateralRefundDelay: SerializedRelativeTimelock;
  unilateralRefundWithoutReceiverDelay: SerializedRelativeTimelock;
  nonInteractiveParameters?: SerializedNonInteractiveParameters;
}

/** The quote surface hosts show before asking the user to commit. */
export interface ArkadeQuoteSummary {
  venue: "arkade-intents";
  route: ArkadeRoute;
  pair: string;
  rfqId: string;
  /** What the user gives, sats. */
  fromAmountSats: number;
  /** What the user receives, sats. */
  toAmountSats: number;
  /** The spread — Intents quotes carry no separate fee field. */
  feeSats: number;
  /** Deadline for the first enforceable commitment, unix seconds. */
  validUntil: number;
  /** Refund horizon, unix seconds (HTLC-class quotes always carry one). */
  refundLocktime?: number;
  solverPubkey: string;
}

/**
 * The persisted recovery record — everything `reconcile()` needs with no
 * live objects: plain JSON throughout, so any store (IndexedDB,
 * `chrome.storage`, SQLite) can hold it verbatim.
 */
export interface ArkadeSwapRecord {
  /** The rfq_id — unique per negotiation, so it is the record key. */
  id: string;
  route: ArkadeRoute;
  phase: ArkadeSwapPhase;
  /** Unix seconds. */
  createdAt: number;
  /** The verified signed quote, as received (already plain JSON). */
  quote: RfqQuote;
  /** The trader's OWN contract derivation. */
  address: string;
  swapPkScriptHex: string;
  scriptOptions: SerializedVhtlcOptions;
  /** The wallet-provisioned secrets for this swap's own leg — the refund
   * key on a send, the claim key (+ preimage material) on a receive. Public;
   * the signer/preimage re-derive from the wallet. `swapSecretsToRecord`'s
   * own output shape. */
  secrets: SwapSecretsProjection & { signingDescriptor: string };
  /** Send-route fields. */
  fundAmountSats?: number;
  refundAddress?: string;
  fundingTxid?: string;
  /** Receive-route fields. */
  invoice?: string;
  payAmountSats?: number;
  expectedAmountSats?: number;
  payoutAddress?: string;
  /** Last moment the hold invoice can be paid, unix seconds. */
  invoiceExpiresAt?: number;
  /** Terminal evidence: the Ark txid that settled or refunded the swap
   * (our own claim/refund push, or the counterparty's spend as observed on
   * chain — in that preference order). */
  resolvedTxid?: string;
  /** Swept outpoints, when `phase === "needs_recovery"`. */
  recoveryOutpoints?: string[];
  /** Why `phase === "failed"`. */
  failureReason?: string;
  /**
   * `RfqSwapManager`'s own per-swap state, mirrored verbatim once this
   * record has been handed to it. Absent while `phase === "prepared"`.
   * Restored on `addSwap` so a process restart resumes with no lost
   * evidence — see `ArkadeIntentsVenue`'s module doc.
   */
  managerState?: RfqSwapState;
  managerRefundArkTxid?: string;
  /** Receive-route only: our own submitted claim's txid. */
  managerClaimArkTxid?: string;
  managerLockupSpendArkTxids?: string[];
}

/** The persistence port. Implementations must write-through before resolving. */
export interface ArkadeSwapStore {
  put(record: ArkadeSwapRecord): Promise<void>;
  get(id: string): Promise<ArkadeSwapRecord | undefined>;
  /** Every record whose phase is `prepared` or `funded`. */
  listPending(): Promise<ArkadeSwapRecord[]>;
}

/** Reference store for tests and short-lived processes. Not restart-safe. */
export class InMemoryArkadeSwapStore implements ArkadeSwapStore {
  private records = new Map<string, ArkadeSwapRecord>();

  async put(record: ArkadeSwapRecord): Promise<void> {
    this.records.set(record.id, { ...record });
  }

  async get(id: string): Promise<ArkadeSwapRecord | undefined> {
    const record = this.records.get(id);
    return record ? { ...record } : undefined;
  }

  async listPending(): Promise<ArkadeSwapRecord[]> {
    return [...this.records.values()]
      .filter((r) => r.phase === "prepared" || r.phase === "funded")
      .map((r) => ({ ...r }));
  }
}

/** What one `reconcile()` pass did, keyed by record id (asset swaps key by
 * funding txid). */
export interface ReconcileReport {
  settled: string[];
  refunded: string[];
  cancelled: string[];
  needsRecovery: string[];
  /** NEW: an action kept failing until its window closed. See
   * {@link ArkadeSwapPhase}'s `failed` case. */
  failed: string[];
  /** Still pending — nothing actionable this pass. */
  pending: string[];
  /** Records whose action threw; the record keeps its previous phase. */
  errors: { id: string; error: unknown }[];
}

/**
 * The flow seam. Defaults to the real `@arkade-os/swap` functions; tests
 * inject fakes here instead of mocking a wallet's contract manager.
 */
export interface ArkadeIntentsFlows {
  requestLightningSend: typeof requestLightningSend;
  requestLightningReceive: typeof requestLightningReceive;
  /** Read the lockup's fate directly — used only by this venue's own
   * pre-manager fast path for a still-`prepared` record past its window
   * (see {@link ArkadeIntentsVenue.reconcile}). Once a record is `funded`,
   * `RfqSwapManager` reads this on its own. */
  readLockupFate: typeof readLockupFate;
  /** Claim a funded receive-route lockup. Defaults to resolving the
   * record's receiver identity and preimage from the wallet and calling
   * `pushClaim`. */
  claimLockup: (
    record: ArkadeSwapRecord,
    script: InstanceType<typeof VHTLC.ScriptV2>,
    vtxos: readonly LockupVtxo[],
    options: { partiallyClaimed: boolean },
  ) => Promise<{ arkTxid: string; amount: number }>;
  /** Push the trader's own `refundWithoutReceiver` for a lockup. Defaults to
   * resolving the record's sender identity from the wallet and calling
   * `pushRefundWithoutReceiver`; resolves `null` for an empty lockup. */
  refundArkade: (
    record: ArkadeSwapRecord,
    script: InstanceType<typeof VHTLC.ScriptV2>,
  ) => Promise<{ arkTxid: string; amount: number } | null>;
  createOffer: typeof createOffer;
  cancelOffer: typeof cancelOffer;
  /** The deposit-spend classifier for asset-swap reconciliation: given the
   * swap and its spent deposit outpoint, name the covenant leaf the spend
   * took. Defaults to the fetch-and-classify recipe over the indexer. */
  classifyAssetSwapSpend: (
    swap: AssetSwap,
    deposit: { txid: string; vout: number; spendTxids: string[] },
  ) => Promise<SpendKind>;
}

export interface ArkadeIntentsVenueOptions {
  wallet: IWallet;
  arkServerUrl: string;
  /** RFQ transport from the solver's card (`nostrRfqTransport`, HTTP, …). */
  transport: RfqTransport;
  store: ArkadeSwapStore;
  /** Enables the intra-Arkade asset-swap route. The ecosystem repository
   * type on purpose (`InMemoryAssetSwapRepository`,
   * `IndexedDbAssetSwapRepository`, or your own): `cancelOffer` and the
   * restore scan are written against it, and a funded offer's recovery net
   * is the chain scan — the offer packet rides the funding tx itself. */
  assetSwapRepository?: AssetSwapRepository;
  /** Defaults to REST providers on `arkServerUrl`. */
  arkProvider?: RefundArkProvider;
  indexerProvider?: LockupSpendIndexer;
  /** Unix seconds; injectable for tests. */
  now?: () => number;
  flows?: Partial<ArkadeIntentsFlows>;
}

/** Result of {@link ArkadeIntentsVenue.prepareAssetSwap}. */
export interface PreparedAssetSwap {
  /** The encoded offer — `cancelOffer`'s only required input. Persisted by
   * {@link ArkadeIntentsVenue.notifyAssetSwapFunded}; until funding it can
   * simply be dropped and re-derived. */
  offerHex: string;
  /** Fund this address… */
  address: string;
  /** …including this packet in `wallet.send`'s `extensions` — it is what
   * makes the funded offer discoverable to solvers. Omit it and the deposit
   * sits indexed by nobody. */
  extension: { type: number; payload: Uint8Array };
  swapPkScriptHex: string;
}

/** Result of {@link ArkadeIntentsVenue.prepareLightningSend}. */
export interface PreparedLightningSend {
  record: ArkadeSwapRecord;
  summary: ArkadeQuoteSummary;
  /** Fund exactly this address with exactly `fundAmountSats` — funding is
   * the quote acceptance; there is no accept message. */
  address: string;
  fundAmountSats: number;
}

/** Result of {@link ArkadeIntentsVenue.prepareLightningReceive}. */
export interface PreparedLightningReceive {
  record: ArkadeSwapRecord;
  summary: ArkadeQuoteSummary;
  /** The solver's hold invoice — paying it arms the swap. */
  invoice: string;
  payAmountSats: number;
  invoiceExpiresAt: number;
}

/**
 * An RFQ transport for the KaleidoSwap maker, from the same `makerUrl` the
 * rest of the SDK is configured with.
 *
 * The maker serves the corridor over HTTP — `POST /v1/swap`,
 * `GET /v1/rfq/{id}` — beside its `/v2` routes, so a venue pointed at
 * `https://maker.signet.kaleidoswap.com/v2` needs a transport rooted at
 * `https://maker.signet.kaleidoswap.com`. {@link corridorRootFromMakerUrl}
 * derives that with the same rule the Rust core applies to its own `/v2`
 * base, so the main entry's `IntentsCorridor` and this venue reach the same
 * origin by construction rather than by two copies of a string.
 *
 * ```ts
 * const venue = new ArkadeIntentsVenue({
 *   wallet, arkServerUrl, store,
 *   transport: kaleidoswapHttpTransport("https://maker.signet.kaleidoswap.com/v2"),
 * });
 * ```
 */
export function kaleidoswapHttpTransport(
  makerUrl: string,
  options?: { fetchImpl?: typeof fetch },
): RfqTransport {
  return httpTransport(corridorRootFromMakerUrl(makerUrl), options);
}

const hex = {
  encode(bytes: Uint8Array): string {
    let out = "";
    for (const byte of bytes) out += byte.toString(16).padStart(2, "0");
    return out;
  },
  decode(value: string): Uint8Array {
    if (value.length % 2 !== 0)
      throw new Error(`odd-length hex: ${value.length}`);
    const out = new Uint8Array(value.length / 2);
    for (let i = 0; i < out.length; i++) {
      const byte = Number.parseInt(value.slice(i * 2, i * 2 + 2), 16);
      if (Number.isNaN(byte)) throw new Error("invalid hex");
      out[i] = byte;
    }
    return out;
  },
};

const base64 = {
  decode(value: string): Uint8Array {
    const raw = atob(value);
    const out = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
    return out;
  },
};

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

type VhtlcOptions = ConstructorParameters<typeof VHTLC.ScriptV2>[0];

export function serializeVhtlcOptions(
  options: VhtlcOptions,
): SerializedVhtlcOptions {
  const timelock = (
    t: VhtlcOptions["unilateralClaimDelay"],
  ): SerializedRelativeTimelock => ({
    type: t.type,
    value: t.value.toString(),
  });
  const covenants = options.nonInteractiveParameters;
  return {
    senderHex: hex.encode(options.sender),
    receiverHex: hex.encode(options.receiver),
    serverHex: hex.encode(options.server),
    preimageHashHex: hex.encode(options.preimageHash),
    refundLocktime: options.refundLocktime.toString(),
    unilateralClaimDelay: timelock(options.unilateralClaimDelay),
    unilateralRefundDelay: timelock(options.unilateralRefundDelay),
    unilateralRefundWithoutReceiverDelay: timelock(
      options.unilateralRefundWithoutReceiverDelay,
    ),
    ...(covenants && {
      nonInteractiveParameters: {
        emulatorPubkeyHex: hex.encode(covenants.emulatorPubkey),
        receiverPkScriptHex: hex.encode(covenants.receiverPkScript),
        senderPkScriptHex: hex.encode(covenants.senderPkScript),
        // Always explicit — see the field's own doc comment for why this is
        // what makes a missing key at decode time unambiguous.
        legacy:
          covenants.legacy === "preTimelockedRefund"
            ? ("preTimelockedRefund" as const)
            : ("current" as const),
      },
    }),
  };
}

export function deserializeVhtlcOptions(
  s: SerializedVhtlcOptions,
): VhtlcOptions {
  const timelock = (t: SerializedRelativeTimelock) => ({
    type: t.type,
    value: BigInt(t.value),
  });
  // A record from before this field existed used two separate objects
  // (`nonInteractiveClaim` / `nonInteractiveRefund`) rather than one
  // `nonInteractiveParameters` — the shape the pre-0.0.4 peer version of
  // `@arkade-os/sdk` exposed. Best-effort migration: that shape can only ever
  // have been the pre-timelocked-refund covenant, since it necessarily
  // predates the leaf `legacy` distinguishes.
  const legacyShape = s as unknown as {
    nonInteractiveClaim?: {
      receiverPkScriptHex: string;
      emulatorPubkeyHex: string;
    };
    nonInteractiveRefund?: {
      senderPkScriptHex: string;
      emulatorPubkeyHex: string;
    };
  };
  const covenants: SerializedNonInteractiveParameters | undefined =
    s.nonInteractiveParameters ??
    (legacyShape.nonInteractiveClaim && legacyShape.nonInteractiveRefund
      ? {
          emulatorPubkeyHex: legacyShape.nonInteractiveClaim.emulatorPubkeyHex,
          receiverPkScriptHex:
            legacyShape.nonInteractiveClaim.receiverPkScriptHex,
          senderPkScriptHex: legacyShape.nonInteractiveRefund.senderPkScriptHex,
          legacy: "preTimelockedRefund",
        }
      : undefined);
  return {
    sender: hex.decode(s.senderHex),
    receiver: hex.decode(s.receiverHex),
    server: hex.decode(s.serverHex),
    preimageHash: hex.decode(s.preimageHashHex),
    refundLocktime: BigInt(s.refundLocktime),
    unilateralClaimDelay: timelock(s.unilateralClaimDelay),
    unilateralRefundDelay: timelock(s.unilateralRefundDelay),
    unilateralRefundWithoutReceiverDelay: timelock(
      s.unilateralRefundWithoutReceiverDelay,
    ),
    ...(covenants && {
      nonInteractiveParameters: {
        emulatorPubkey: hex.decode(covenants.emulatorPubkeyHex),
        receiverPkScript: hex.decode(covenants.receiverPkScriptHex),
        senderPkScript: hex.decode(covenants.senderPkScriptHex),
        // Never default to the current (fuller) shape for silently-missing
        // data: a lockup already funded in the legacy shape recomputes a
        // different taproot address under the current one, and that address
        // would not be the one anyone sent to.
        ...((covenants.legacy ?? "preTimelockedRefund") ===
        "preTimelockedRefund"
          ? { legacy: "preTimelockedRefund" as const }
          : {}),
      },
    }),
  };
}

/**
 * `@arkade-os/swap` 0.0.20 widened quote amounts to `number | string`: the
 * string form is the canonical decimal of an asset leg. Both routes here are
 * sats corridors, so a string is a malformed quote — refuse it rather than
 * coerce a value nobody should fund.
 */
function quoteSats(
  quote: RfqQuote,
  field: "from_amount" | "to_amount",
): number {
  const value = quote[field];
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`quote ${field} is not a sats amount: ${String(value)}`);
  }
  return value;
}

function quoteSummary(route: ArkadeRoute, quote: RfqQuote): ArkadeQuoteSummary {
  const fromAmountSats = quoteSats(quote, "from_amount");
  const toAmountSats = quoteSats(quote, "to_amount");
  return {
    venue: "arkade-intents",
    route,
    pair: quote.pair,
    rfqId: quote.rfq_id,
    fromAmountSats,
    toAmountSats,
    feeSats: fromAmountSats - toAmountSats,
    validUntil: quote.valid_until,
    refundLocktime: quote.refund_locktime,
    solverPubkey: quote.solver_pubkey,
  };
}

/**
 * The venue. One instance per (wallet, solver transport) pair; construction
 * does no I/O.
 */
export class ArkadeIntentsVenue {
  private readonly wallet: IWallet;
  private readonly arkServerUrl: string;
  private readonly transport: RfqTransport;
  private readonly store: ArkadeSwapStore;
  private readonly ark: RefundArkProvider;
  private readonly indexer: LockupSpendIndexer;
  private readonly now: () => number;
  private readonly flows: ArkadeIntentsFlows;
  private readonly assetSwaps?: AssetSwapRepository;

  /** The state machine. Never `start()`/`stop()`-ed — see the module doc:
   * this venue owns no timer, and drives the manager itself via
   * `addSwap()`/`poll()` from {@link reconcile}, exactly like the host's own
   * scheduler used to drive the old hand-rolled reconcile loop. */
  private readonly manager: RfqSwapManager;
  /** rfqId → outpoints from the most recent `LockupNeedsRecoveryError` this
   * process observed for that swap. Informational: it drives this venue's
   * own `needs_recovery` phase label; the manager itself keeps retrying
   * regardless. Cleared once the swap reaches a state that is not one of
   * this label's own precondition (see {@link phaseOf}). */
  private readonly recoveryOutpoints = new Map<string, string[]>();

  constructor(options: ArkadeIntentsVenueOptions) {
    this.wallet = options.wallet;
    this.arkServerUrl = options.arkServerUrl;
    this.transport = options.transport;
    this.store = options.store;
    this.assetSwaps = options.assetSwapRepository;
    this.ark = options.arkProvider ?? new RestArkProvider(options.arkServerUrl);
    this.indexer =
      options.indexerProvider ?? new RestIndexerProvider(options.arkServerUrl);
    this.now = options.now ?? (() => Math.floor(Date.now() / 1000));
    this.flows = {
      requestLightningSend,
      requestLightningReceive,
      readLockupFate,
      claimLockup: (record, script, vtxos, claimOptions) =>
        this.defaultClaimLockup(record, script, vtxos, claimOptions),
      refundArkade: (record, script) =>
        this.defaultRefundArkade(record, script),
      createOffer,
      cancelOffer,
      classifyAssetSwapSpend: (swap, deposit) =>
        this.fetchAndClassifySpend(swap, deposit),
      ...options.flows,
    };

    this.manager = new RfqSwapManager(
      { indexer: this.indexer },
      { enableAutoActions: true, now: this.now },
    );
    const callbacks: AvailableRfqSwapManagerCallbacks = {
      claimLockup: (swap, vtxos, claimOptions) =>
        this.dispatchClaimLockup(swap, vtxos, claimOptions),
      refundArkade: (swap) => this.dispatchRefundArkade(swap),
      saveSwap: (swap) => this.managerSaveSwap(swap),
    };
    this.manager.setCallbacks(callbacks);
    this.manager.onSwapFailed((swap, error) => {
      if (swap.kind === "onchain_send") return; // never added by this venue
      if (error instanceof LockupNeedsRecoveryError) {
        this.recoveryOutpoints.set(swap.rfqId, [...error.outpoints]);
        // The manager may not have marked this swap dirty for a caught-and-
        // retried failure (see the module doc on `ReconcileReport.errors`),
        // so this label would otherwise lag until the swap's state next
        // changes for an unrelated reason. Force the write.
        void this.managerSaveSwap(swap).catch(() => {});
      }
    });
  }

  /**
   * Quote and derive the `arkade:BTC -> lightning:BTC` swap. The record is
   * persisted before this returns; the caller then funds `address` with
   * `fundAmountSats` before `summary.validUntil` and reports the txid via
   * {@link notifyFunded}. After funding the wallet can go offline — the
   * solver observes the lockup, pays the invoice, and claims.
   */
  async prepareLightningSend(params: {
    invoice: InvoiceFacts;
    emulatorPubkey?: string;
  }): Promise<PreparedLightningSend> {
    const send = await this.flows.requestLightningSend(
      this.wallet,
      this.arkServerUrl,
      this.transport,
      { invoice: params.invoice, emulatorPubkey: params.emulatorPubkey },
    );
    const record: ArkadeSwapRecord = {
      id: send.rfqId,
      route: "arkade:BTC->lightning:BTC",
      phase: "prepared",
      createdAt: this.now(),
      quote: send.quote,
      address: send.address,
      swapPkScriptHex: hex.encode(send.swapPkScript),
      scriptOptions: serializeVhtlcOptions(send.script.options),
      secrets: swapSecretsToRecord(send.secrets),
      fundAmountSats: send.fundAmount,
      refundAddress: send.refundAddress,
    };
    await this.store.put(record);
    return {
      record,
      summary: quoteSummary(record.route, send.quote),
      address: send.address,
      fundAmountSats: send.fundAmount,
    };
  }

  /**
   * Quote and derive the `lightning:BTC -> arkade:BTC` swap. The record is
   * persisted before this returns. Paying `invoice` is the acceptance; call
   * {@link notifyFunded} once the caller has dispatched the payment, and
   * claim via {@link claimReceive} (or let {@link reconcile} do it) — the
   * swap completes only when the claim reveals the preimage.
   */
  async prepareLightningReceive(params: {
    amountSats: number;
    /** `"to"` = amount received on Arkade (default); `"from"` = amount paid. */
    amountSide?: "from" | "to";
    /** covclaimd's compressed pubkey; enables offline claim via the solver's
     * claim daemon. Optional — unset where no covclaimd is deployed. */
    covclaimdPubkey?: Uint8Array;
    /** The host's own BOLT11 decoder, applied to the SOLVER's invoice. */
    decodeInvoice: (bolt11: string) => InvoiceFacts;
    maxPayAmountSats?: number;
    emulatorPubkey?: string;
  }): Promise<PreparedLightningReceive> {
    const receive = await this.flows.requestLightningReceive(
      this.wallet,
      this.arkServerUrl,
      this.transport,
      {
        amount: params.amountSats,
        amountSide: params.amountSide ?? "to",
        covclaimdPubkey: params.covclaimdPubkey,
        decodeInvoice: params.decodeInvoice,
        maxPayAmount: params.maxPayAmountSats,
        emulatorPubkey: params.emulatorPubkey,
      },
    );
    const record: ArkadeSwapRecord = {
      id: receive.rfqId,
      route: "lightning:BTC->arkade:BTC",
      phase: "prepared",
      createdAt: this.now(),
      quote: receive.quote,
      address: receive.address,
      swapPkScriptHex: hex.encode(receive.swapPkScript),
      scriptOptions: serializeVhtlcOptions(receive.script.options),
      secrets: swapSecretsToRecord(receive.secrets),
      invoice: receive.invoice,
      payAmountSats: receive.payAmount,
      expectedAmountSats: receive.expectedAmount,
      payoutAddress: receive.payoutAddress,
      invoiceExpiresAt: receive.invoiceExpiresAt,
    };
    await this.store.put(record);
    return {
      record,
      summary: quoteSummary(record.route, receive.quote),
      invoice: receive.invoice,
      payAmountSats: receive.payAmount,
      invoiceExpiresAt: receive.invoiceExpiresAt,
    };
  }

  /**
   * Record the caller's commitment: the send-lockup funding txid, or (for
   * receives, txid omitted) that the hold-invoice payment was dispatched.
   * Hands the swap to the internal `RfqSwapManager` from here on.
   *
   * Only `prepared` records advance (repeating on an already-`funded`
   * record is a harmless idempotent retry); a terminal record refuses —
   * re-entering it into the pending set would resurrect a finished swap.
   */
  async notifyFunded(
    id: string,
    fundingTxid?: string,
  ): Promise<ArkadeSwapRecord> {
    const record = await this.mustGet(id);
    if (record.phase !== "prepared" && record.phase !== "funded") {
      throw new Error(
        `notifyFunded: swap ${id} is ${record.phase}, not prepared/funded`,
      );
    }
    record.phase = "funded";
    if (fundingTxid !== undefined) record.fundingTxid = fundingTxid;
    await this.store.put(record);
    await this.ensureTracked(record);
    return record;
  }

  /**
   * Claim a receive-route lockup, revealing the preimage — the step that
   * completes the swap. Triggers manager passes for up to `waitSeconds`
   * (default 10, wall-clock) waiting for the solver's funding to appear and
   * the claim to land; the claim itself remains bounded by the quote's
   * `refund_locktime`, enforced by the manager on every pass regardless of
   * this call.
   */
  async claimReceive(
    id: string,
    options?: { waitSeconds?: number },
  ): Promise<ArkadeSwapRecord> {
    const record = await this.mustGet(id);
    if (record.route !== "lightning:BTC->arkade:BTC") {
      throw new Error(`claimReceive: ${id} is not a receive swap`);
    }
    if (record.phase === "settled") return record;
    await this.ensureTracked(record);
    const waitMs = Math.max(0, options?.waitSeconds ?? 10) * 1000;
    const deadline = Date.now() + waitMs;
    for (;;) {
      await this.manager.poll();
      const updated = await this.mustGet(id);
      if (updated.phase !== "funded") return updated;
      const remaining = deadline - Date.now();
      if (remaining <= 0) return updated;
      await sleep(Math.min(250, remaining));
    }
  }

  /**
   * Resolve a stalled send: triggers a manager pass, which settles as soon
   * as the solver claims or refunds, and otherwise pushes the trader's
   * `refundWithoutReceiver` once `refund_locktime` has matured. Safe to call
   * repeatedly and late.
   */
  async refundSend(id: string): Promise<ArkadeSwapRecord> {
    const record = await this.mustGet(id);
    if (record.route !== "arkade:BTC->lightning:BTC") {
      throw new Error(`refundSend: ${id} is not a send swap`);
    }
    await this.ensureTracked(record);
    await this.manager.poll();
    return this.mustGet(id);
  }

  /** In-flight reconcile pass; a second caller joins it instead of racing. */
  private reconcilePass: Promise<ReconcileReport> | null = null;

  /**
   * One evidence-driven pass over every pending record. Never throws for a
   * single record's failure — errors are reported and the record keeps its
   * phase for the next pass. Designed to be called from a host alarm/timer;
   * this venue owns no timer of its own, and neither does the underlying
   * `RfqSwapManager` from this venue's point of view — it is driven purely
   * through `addSwap()`/`poll()` here, never `start()`/`stop()`.
   *
   * Re-entrant calls share the running pass: a pass can spend time waiting
   * on network reads, and a host alarm firing meanwhile must not run a
   * second pass over the same records.
   *
   * A `prepared` record past its commitment window is checked directly
   * against chain evidence (not yet handed to the manager, since the
   * manager has no notion of "quoted but maybe never funded"): a lockup the
   * indexer has never seen is `cancelled`; any other evidence means the
   * caller's `notifyFunded` call was lost (crash between broadcast and
   * notify) and the record self-heals to `funded` and is handed off.
   */
  reconcile(): Promise<ReconcileReport> {
    if (this.reconcilePass) return this.reconcilePass;
    this.reconcilePass = this.reconcileOnce().finally(() => {
      this.reconcilePass = null;
    });
    return this.reconcilePass;
  }

  private async reconcileOnce(): Promise<ReconcileReport> {
    const report: ReconcileReport = {
      settled: [],
      refunded: [],
      cancelled: [],
      needsRecovery: [],
      failed: [],
      pending: [],
      errors: [],
    };
    const touched: string[] = [];
    for (const record of await this.store.listPending()) {
      try {
        if (record.phase === "prepared") {
          const after = await this.reconcilePreparedRecord(record);
          if (after.phase !== "funded") {
            this.file(report, after);
            continue;
          }
          touched.push(after.id);
          continue;
        }
        await this.ensureTracked(record);
        touched.push(record.id);
      } catch (error) {
        report.errors.push({ id: record.id, error });
      }
    }
    await this.manager.poll();
    for (const id of touched) {
      const record = await this.store.get(id);
      if (record) this.file(report, record);
    }
    if (this.assetSwaps) await this.reconcileAssetSwaps(report);
    return report;
  }

  private file(report: ReconcileReport, record: ArkadeSwapRecord): void {
    switch (record.phase) {
      case "settled":
        report.settled.push(record.id);
        break;
      case "refunded":
        report.refunded.push(record.id);
        break;
      case "cancelled":
        report.cancelled.push(record.id);
        break;
      case "needs_recovery":
        report.needsRecovery.push(record.id);
        break;
      case "failed":
        report.failed.push(record.id);
        break;
      default:
        report.pending.push(record.id);
    }
  }

  /**
   * The pre-manager fast path for a `prepared` record past its window.
   * Chain evidence — never the local flag — decides which way it went: the
   * host may have broadcast the lockup (or dispatched the LN payment) and
   * died before calling `notifyFunded`.
   */
  private async reconcilePreparedRecord(
    record: ArkadeSwapRecord,
  ): Promise<ArkadeSwapRecord> {
    const deadline =
      record.route === "arkade:BTC->lightning:BTC"
        ? record.quote.valid_until
        : (record.invoiceExpiresAt ?? record.quote.valid_until);
    if (this.now() < deadline) return record;

    let fate: LockupFate;
    try {
      fate = await this.flows.readLockupFate(this.indexer, {
        swapPkScript: hex.decode(record.swapPkScriptHex),
        paymentHash: this.paymentHashOf(record),
      });
    } catch {
      // Transient: try again next pass rather than guessing.
      return record;
    }
    if (fate.fate === "unknown") {
      record.phase = "cancelled";
      await this.store.put(record);
      return record;
    }
    // Something is there: the swap secretly got funded. Hand it to the
    // manager from here on — the caller's own reconcile loop (this method's
    // own caller) tracks and polls it the same pass.
    record.phase = "funded";
    await this.store.put(record);
    return record;
  }

  private async ensureTracked(record: ArkadeSwapRecord): Promise<void> {
    if (await this.manager.hasSwap(record.id)) return;
    await this.manager.addSwap(this.liveSwapFromRecord(record));
  }

  /** Rebuild the manager's live swap object from our own persisted record —
   * the composition the manager's own doc recommends for a consumer keeping
   * its own store rather than wiring `RfqSwapManagerDeps.repository`. */
  private liveSwapFromRecord(
    record: ArkadeSwapRecord,
  ): LightningSendSwap | LightningReceiveSwap {
    const script = new VHTLC.ScriptV2(
      deserializeVhtlcOptions(record.scriptOptions),
    );
    const common = {
      rfqId: record.id,
      state: record.managerState ?? ("pending" as RfqSwapState),
      lockupPkScript: hex.decode(record.swapPkScriptHex),
      lockup: { script, address: record.address },
      paymentHash: this.paymentHashOf(record),
      refundLocktime: this.refundLocktimeOf(record),
      createdAt: record.createdAt,
      updatedAt: record.createdAt,
      ...(record.managerRefundArkTxid
        ? { refundArkTxid: record.managerRefundArkTxid }
        : {}),
      ...(record.managerLockupSpendArkTxids?.length
        ? { lockupSpendArkTxids: [...record.managerLockupSpendArkTxids] }
        : {}),
      ...(record.failureReason ? { failure: record.failureReason } : {}),
    };
    if (record.route === "arkade:BTC->lightning:BTC") {
      return { ...common, kind: "lightning_send" };
    }
    return {
      ...common,
      kind: "lightning_receive",
      expectedAmount: this.expectedAmountOf(record),
      ...(record.managerClaimArkTxid
        ? { claimArkTxid: record.managerClaimArkTxid }
        : {}),
    };
  }

  /**
   * `RfqSwapManagerCallbacks.saveSwap` — translate the manager's live state
   * back into this venue's own persisted phase and write it through.
   */
  private async managerSaveSwap(swap: RfqSwap): Promise<void> {
    if (swap.kind === "onchain_send") {
      // Never reachable: this venue never calls `addSwap` with this kind.
      throw new Error("ArkadeIntentsVenue never monitors onchain-send swaps");
    }
    const record = await this.mustGet(swap.rfqId);
    record.managerState = swap.state;
    record.managerRefundArkTxid = swap.refundArkTxid;
    if (swap.kind === "lightning_receive") {
      record.managerClaimArkTxid = swap.claimArkTxid;
    }
    record.managerLockupSpendArkTxids = swap.lockupSpendArkTxids
      ? [...swap.lockupSpendArkTxids]
      : undefined;
    record.failureReason = swap.failure;

    const recovery = this.recoveryOutpoints.get(swap.rfqId);
    record.phase = this.phaseOf(swap, recovery);
    if (record.phase === "needs_recovery" && recovery) {
      record.recoveryOutpoints = recovery;
    } else {
      this.recoveryOutpoints.delete(swap.rfqId);
    }

    const terminalTxid =
      swap.kind === "lightning_receive"
        ? ((swap.state === "refunded"
            ? swap.refundArkTxid
            : swap.claimArkTxid) ?? swap.lockupSpendArkTxids?.[0])
        : (swap.refundArkTxid ?? swap.lockupSpendArkTxids?.[0]);
    if (terminalTxid) record.resolvedTxid = terminalTxid;

    await this.store.put(record);
  }

  /**
   * The manager's `RfqSwapState` projected onto this venue's own phase
   * vocabulary.
   *
   * `refunded` with NO txid evidence anywhere (neither our own push nor a
   * chain-observed spend) means the lockup was never funded at all — the
   * manager settles for `refunded` there too (see `RfqSwapManager`'s own
   * doc, "the ONE place the manager settles for less than proof"), but this
   * venue's older vocabulary calls that `cancelled` instead, since nothing
   * was ever at stake.
   */
  private phaseOf(
    swap: LightningSendSwap | LightningReceiveSwap,
    recovery: string[] | undefined,
  ): ArkadeSwapPhase {
    switch (swap.state) {
      case "settled":
        return "settled";
      case "refunded": {
        const hasEvidence =
          Boolean(swap.refundArkTxid) ||
          Boolean(swap.lockupSpendArkTxids?.length) ||
          (swap.kind === "lightning_receive" && Boolean(swap.claimArkTxid));
        return hasEvidence ? "refunded" : "cancelled";
      }
      case "failed":
        return "failed";
      default:
        return recovery ? "needs_recovery" : "funded";
    }
  }

  /** `RfqSwapManagerCallbacks.claimLockup`, dispatched to the flow seam. */
  private async dispatchClaimLockup(
    swap: LightningReceiveSwap,
    vtxos: readonly LockupVtxo[],
    options: { partiallyClaimed: boolean },
  ): Promise<{ arkTxid: string; amount: number }> {
    const record = await this.mustGet(swap.rfqId);
    const script =
      swap.lockup?.script ??
      new VHTLC.ScriptV2(deserializeVhtlcOptions(record.scriptOptions));
    return this.flows.claimLockup(record, script, vtxos, options);
  }

  /** `RfqSwapManagerCallbacks.refundArkade`, dispatched to the flow seam. */
  private async dispatchRefundArkade(
    swap: RfqSwap,
  ): Promise<{ arkTxid: string; amount: number } | null> {
    if (swap.kind === "onchain_send") {
      throw new Error("onchain-send swaps are not supported by this venue");
    }
    const record = await this.mustGet(swap.rfqId);
    const script =
      swap.lockup?.script ??
      new VHTLC.ScriptV2(deserializeVhtlcOptions(record.scriptOptions));
    return this.flows.refundArkade(record, script);
  }

  private async defaultClaimLockup(
    record: ArkadeSwapRecord,
    script: InstanceType<typeof VHTLC.ScriptV2>,
    vtxos: readonly LockupVtxo[],
    options: { partiallyClaimed: boolean },
  ): Promise<{ arkTxid: string; amount: number }> {
    const receiver = await contractSigner(
      this.wallet,
      this.requireSigningDescriptor(record),
    );
    const preimage = await preimageForSwapRecord(this.wallet, {
      ...record.secrets,
      paymentHash: this.paymentHashOf(record),
    });
    return pushClaim(this.ark, {
      script,
      receiver,
      preimage,
      vtxos,
      destinationPkScript: ArkAddress.decode(this.payoutAddressOf(record))
        .pkScript,
      expectedAmount: this.expectedAmountOf(record),
      partiallyClaimed: options.partiallyClaimed,
    });
  }

  private async defaultRefundArkade(
    record: ArkadeSwapRecord,
    script: InstanceType<typeof VHTLC.ScriptV2>,
  ): Promise<{ arkTxid: string; amount: number } | null> {
    const vtxos = await findLockupVtxos(
      this.indexer,
      hex.decode(record.swapPkScriptHex),
    );
    if (vtxos.length === 0) return null;
    const sender = await senderIdentityForSwapRecord(
      this.wallet,
      record.secrets,
    );
    return pushRefundWithoutReceiver(this.ark, { script, sender, vtxos });
  }

  private async mustGet(id: string): Promise<ArkadeSwapRecord> {
    const record = await this.store.get(id);
    if (!record) throw new Error(`unknown swap record: ${id}`);
    return record;
  }

  private requireSigningDescriptor(record: ArkadeSwapRecord): string {
    if (!record.secrets.signingDescriptor) {
      throw new PreimageNotRecoverableError(
        "no-secrets",
        `swap ${record.id}: no signing descriptor on record`,
      );
    }
    return record.secrets.signingDescriptor;
  }

  private refundLocktimeOf(record: ArkadeSwapRecord): number {
    // The quote carries the binding value; the covenant's own locktime is
    // the fallback — the script is the enforcement, so a record whose quote
    // somehow lacks the field still terminates instead of erroring on
    // every reconcile pass forever.
    return (
      record.quote.refund_locktime ??
      Number(record.scriptOptions.refundLocktime)
    );
  }

  private payoutAddressOf(record: ArkadeSwapRecord): string {
    if (!record.payoutAddress)
      throw new Error(`swap ${record.id}: no payout address`);
    return record.payoutAddress;
  }

  private expectedAmountOf(record: ArkadeSwapRecord): number {
    if (record.expectedAmountSats === undefined) {
      throw new Error(`swap ${record.id}: no expected amount`);
    }
    return record.expectedAmountSats;
  }

  private paymentHashOf(record: ArkadeSwapRecord): string {
    const paymentHash = record.quote.profile?.payment_hash;
    if (typeof paymentHash !== "string") {
      throw new Error(`swap ${record.id}: quote profile has no payment_hash`);
    }
    return paymentHash;
  }

  // ─── Intra-Arkade asset swaps ────────────────────────────────────────────

  /**
   * Derive the non-interactive asset-swap covenant (BTC ↔ Arkade asset).
   *
   * Unlike the corridor `prepare*` calls this persists NOTHING, mirroring
   * the upstream design it wraps: `createOffer` is pure derivation plus a
   * contract-manager registration, so before funding there is nothing at
   * stake and a dropped result is simply re-derived. The record is written
   * by {@link notifyAssetSwapFunded} — and even a crash between funding and
   * that call is recoverable, because the offer packet rides the funding
   * transaction itself (the restore scan rebuilds the record from chain).
   *
   * Fund `address` including `extension` in `wallet.send`'s `extensions` —
   * without the packet the deposit is invisible to every solver.
   */
  async prepareAssetSwap(params: {
    /** The covenant's floor: a fill must deliver at least this. */
    wantAmountAtomic: bigint;
    /** Set exactly one: the asset bought (deposit is BTC)… */
    wantAssetId?: string;
    /** …or the asset sold (payout is BTC sats). */
    offerAssetId?: string;
    emulatorPubkey?: string;
  }): Promise<PreparedAssetSwap> {
    const offer = await this.flows.createOffer(this.wallet, this.arkServerUrl, {
      wantAmount: params.wantAmountAtomic,
      wantAsset: params.wantAssetId
        ? asset.AssetId.fromString(params.wantAssetId)
        : undefined,
      offerAsset: params.offerAssetId
        ? asset.AssetId.fromString(params.offerAssetId)
        : undefined,
      emulatorPubkey: params.emulatorPubkey,
    });
    return {
      offerHex: offer.offerHex,
      address: offer.address,
      extension: offer.extension,
      swapPkScriptHex: hex.encode(offer.swapPkScript),
    };
  }

  /**
   * Persist the funded offer. The funding txid — not the address — is the
   * swap's identity: identical terms derive the identical address, so two
   * deposits can share one address and only the txid tells them apart.
   */
  async notifyAssetSwapFunded(input: {
    prepared: PreparedAssetSwap;
    fundingTxid: string;
    /** 'btc' or a 68-hex asset id, per the ecosystem record shape. */
    fromAssetId: string;
    toAssetId: string;
    fromAmountAtomic: bigint;
    toAmountAtomic: bigint;
  }): Promise<AssetSwap> {
    const repository = this.assetSwapsOrThrow();
    const swap: AssetSwap = {
      id: input.fundingTxid,
      fromAsset: input.fromAssetId,
      toAsset: input.toAssetId,
      fromAmount: input.fromAmountAtomic.toString(),
      toAmount: input.toAmountAtomic.toString(),
      swapAddress: input.prepared.address,
      swapPkScript: input.prepared.swapPkScriptHex,
      offerHex: input.prepared.offerHex,
      fundingTxid: input.fundingTxid,
      status: "pending",
      createdAt: this.now() * 1000,
    };
    await addAssetSwap(repository, swap);
    return swap;
  }

  /**
   * Cancel an open offer — no solver signature, no timeout to wait out; an
   * unfilled offer never expires, so this is the ONLY exit. Cancellation
   * races a fill: when the deposit is already spent this classifies the
   * spend instead of failing, and a race lost to the solver reports the
   * swap `fulfilled` — a success, not an error.
   */
  async cancelAssetSwap(fundingTxid: string): Promise<AssetSwap> {
    const repository = this.assetSwapsOrThrow();
    const swap = (await getAssetSwaps(repository)).find(
      (s) => s.id === fundingTxid,
    );
    if (!swap) throw new Error(`unknown asset swap: ${fundingTxid}`);
    try {
      // Writes the cancelling → cancelled transition into the repository
      // itself; nothing to persist here on success.
      await this.flows.cancelOffer(
        this.wallet,
        this.arkServerUrl,
        swap.offerHex,
        {
          repository,
          fundingTxid: swap.fundingTxid,
          swapAddress: swap.swapAddress,
        },
      );
    } catch (error) {
      // A spent deposit means the race resolved without us — classify it
      // rather than guessing, and rethrow only when the chain answers
      // nothing (a transient failure the next reconcile pass retries).
      const updated = await this.reconcileAssetSwap(swap);
      if (updated.status === "pending" || updated.status === "cancelling") {
        throw error;
      }
      return updated;
    }
    const after = (await getAssetSwaps(repository)).find(
      (s) => s.id === fundingTxid,
    );
    return after ?? swap;
  }

  private assetSwapsOrThrow(): AssetSwapRepository {
    if (!this.assetSwaps) {
      throw new Error(
        "asset swaps need an assetSwapRepository on the venue options",
      );
    }
    return this.assetSwaps;
  }

  /** One polling pass over the repository's live asset swaps — the
   * alarm-friendly stand-in for `watchOfferSwaps`, which needs a live
   * contract-event stream this venue deliberately does not own. */
  private async reconcileAssetSwaps(report: ReconcileReport): Promise<void> {
    const repository = this.assetSwapsOrThrow();
    for (const swap of await getAssetSwaps(repository)) {
      if (swap.status !== "pending" && swap.status !== "cancelling") continue;
      try {
        const updated = await this.reconcileAssetSwap(swap);
        switch (updated.status) {
          case "fulfilled":
            report.settled.push(swap.id);
            break;
          case "cancelled":
            report.cancelled.push(swap.id);
            break;
          case "recoverable":
            report.needsRecovery.push(swap.id);
            break;
          default:
            report.pending.push(swap.id);
        }
      } catch (error) {
        report.errors.push({ id: swap.id, error });
      }
    }
  }

  /** Classify one swap's deposit from chain evidence and persist any
   * transition. An unspent or unfound deposit changes nothing — never
   * guess; a later pass decides. */
  private async reconcileAssetSwap(swap: AssetSwap): Promise<AssetSwap> {
    const repository = this.assetSwapsOrThrow();
    const { vtxos } = await this.indexer.getVtxos({
      scripts: [swap.swapPkScript],
    });
    const deposit = (vtxos ?? []).find(
      (v: { txid: string }) => v.txid === swap.fundingTxid,
    );
    if (!deposit) return swap;
    const spent = Boolean(
      deposit.isSpent || deposit.spentBy || deposit.settledBy,
    );
    if (!spent) return swap;
    const spendTxids = spendTxidsOf(deposit);
    const kind = await this.flows.classifyAssetSwapSpend(swap, {
      txid: deposit.txid,
      vout: deposit.vout,
      spendTxids,
    });
    if (kind === "indeterminate") return swap;
    const status = kind === "fulfilled" ? "fulfilled" : "cancelled";
    const spentTxid = spendTxids[0];
    await updateAssetSwap(repository, swap.id, {
      status,
      spentTxid,
      completedAt: this.now() * 1000,
    });
    return { ...swap, status, spentTxid, completedAt: this.now() * 1000 };
  }

  /** The default `classifyAssetSwapSpend`: fetch the candidate spending
   * transactions and read which covenant leaf the spend took — the same
   * recipe the upstream watcher and restore scan use. */
  private async fetchAndClassifySpend(
    swap: AssetSwap,
    deposit: { txid: string; vout: number; spendTxids: string[] },
  ): Promise<SpendKind> {
    if (deposit.spendTxids.length === 0) return "indeterminate";
    const info = await this.ark.getInfo();
    // 33-byte compressed hex on the wire; the covenant wants x-only.
    const serverPubkey = hex.decode(info.signerPubkey).slice(1);
    const { txs } = await this.indexer.getVirtualTxs(deposit.spendTxids);
    const parsed = (txs ?? []).flatMap((psbt: string) => {
      try {
        return [Transaction.fromPSBT(base64.decode(psbt))];
      } catch {
        return [];
      }
    });
    return classifyDepositSpend(
      decodeOffer(hex.decode(swap.offerHex)),
      serverPubkey,
      parsed,
      { txid: deposit.txid, vout: deposit.vout },
    );
  }
}
