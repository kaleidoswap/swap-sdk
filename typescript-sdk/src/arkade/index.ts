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
 * ## Version coupling
 *
 * `@arkade-os/swap` hard-pins its own `@arkade-os/sdk`; the `wallet` object
 * crossing this boundary must come from that same SDK line. The peer ranges
 * encode it: `>=0.4.60 <0.5.0` for the SDK (0.4.60 introduced
 * `VHTLC.ScriptV2`; 0.5.x is uncharted and excluded on purpose) and
 * `^0.0.3` for `@arkade-os/swap` — which under 0.0.x semver rules resolves
 * to exactly 0.0.3, intentionally, while its API is pre-stability. The host
 * app owns the pins.
 */

import type { IWallet } from "@arkade-os/sdk";
import {
  ArkAddress,
  RestArkProvider,
  RestIndexerProvider,
  Transaction,
  VHTLC,
  asset,
} from "@arkade-os/sdk";
import type {
  AssetSwap,
  AssetSwapRepository,
  InvoiceFacts,
  LockupFate,
  RefundArkProvider,
  RefundIndexer,
  RfqQuote,
  RfqTransport,
  SpendKind,
  SwapSecrets,
} from "@arkade-os/swap";
import {
  addAssetSwap,
  cancelOffer,
  claimReceiveLockup,
  classifyDepositSpend,
  createOffer,
  decodeOffer,
  getAssetSwaps,
  preimageForRfqSecrets,
  readLockupFate,
  refundIfUnresolved,
  requestLightningReceive,
  requestLightningSend,
  rfqSecretsOfRecord,
  rfqSecretsToRecord,
  senderIdentityForRfqSecrets,
  spendTxidsOf,
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
 *            receive: hold invoice reported paid by the caller).
 * `settled` — evidence-terminal success (send: lockup claimed with the
 *            preimage; receive: we claimed the solver's lockup).
 * `refunded` — the commitment came back (timeout refund, or solver refund).
 * `cancelled` — nothing ever became enforceable (quote or invoice expired
 *            unfunded, or the claim window closed before funding).
 * `needs_recovery` — funds exist but the lockup's batch was swept; the
 *            wallet's VTXO recovery must run before a refund can be pushed.
 */
export type ArkadeSwapPhase =
  | "prepared"
  | "funded"
  | "settled"
  | "refunded"
  | "cancelled"
  | "needs_recovery";

/** JSON-safe form of the SDK's `RelativeTimelock` (`bigint` → decimal string). */
export interface SerializedRelativeTimelock {
  type: "blocks" | "seconds";
  value: string;
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
  nonInteractiveClaim?: {
    receiverPkScriptHex: string;
    emulatorPubkeyHex: string;
  };
  nonInteractiveRefund?: {
    senderPkScriptHex: string;
    emulatorPubkeyHex: string;
  };
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
  /** `rfqSecretsToRecord` output — a public descriptor on HD wallets. */
  secrets: ReturnType<typeof rfqSecretsToRecord>;
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
  /** Terminal evidence: the Ark txid that settled or refunded the swap. */
  resolvedTxid?: string;
  /** Swept outpoints, when `phase === "needs_recovery"`. */
  recoveryOutpoints?: string[];
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
  claimReceiveLockup: typeof claimReceiveLockup;
  refundIfUnresolved: typeof refundIfUnresolved;
  readLockupFate: typeof readLockupFate;
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
  indexerProvider?: RefundIndexer & Parameters<typeof readLockupFate>[0];
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
    ...(options.nonInteractiveClaim && {
      nonInteractiveClaim: {
        receiverPkScriptHex: hex.encode(
          options.nonInteractiveClaim.receiverPkScript,
        ),
        emulatorPubkeyHex: hex.encode(
          options.nonInteractiveClaim.emulatorPubkey,
        ),
      },
    }),
    ...(options.nonInteractiveRefund && {
      nonInteractiveRefund: {
        senderPkScriptHex: hex.encode(
          options.nonInteractiveRefund.senderPkScript,
        ),
        emulatorPubkeyHex: hex.encode(
          options.nonInteractiveRefund.emulatorPubkey,
        ),
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
    ...(s.nonInteractiveClaim && {
      nonInteractiveClaim: {
        receiverPkScript: hex.decode(s.nonInteractiveClaim.receiverPkScriptHex),
        emulatorPubkey: hex.decode(s.nonInteractiveClaim.emulatorPubkeyHex),
      },
    }),
    ...(s.nonInteractiveRefund && {
      nonInteractiveRefund: {
        senderPkScript: hex.decode(s.nonInteractiveRefund.senderPkScriptHex),
        emulatorPubkey: hex.decode(s.nonInteractiveRefund.emulatorPubkeyHex),
      },
    }),
  };
}

function quoteSummary(route: ArkadeRoute, quote: RfqQuote): ArkadeQuoteSummary {
  const spread = quote.from_amount - quote.to_amount;
  return {
    venue: "arkade-intents",
    route,
    pair: quote.pair,
    rfqId: quote.rfq_id,
    fromAmountSats: quote.from_amount,
    toAmountSats: quote.to_amount,
    feeSats: spread,
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
  private readonly indexer: RefundIndexer &
    Parameters<typeof readLockupFate>[0];
  private readonly now: () => number;
  private readonly flows: ArkadeIntentsFlows;
  private readonly assetSwaps?: AssetSwapRepository;

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
      claimReceiveLockup,
      refundIfUnresolved,
      readLockupFate,
      createOffer,
      cancelOffer,
      classifyAssetSwapSpend: (swap, deposit) =>
        this.fetchAndClassifySpend(swap, deposit),
      ...options.flows,
    };
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
      secrets: rfqSecretsToRecord(send.secrets),
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
     * claim daemon. Required by the underlying flow. */
    covclaimdPubkey: Uint8Array;
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
      secrets: rfqSecretsToRecord(receive.secrets),
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
    return record;
  }

  /**
   * Claim a receive-route lockup, revealing the preimage — the step that
   * completes the swap. Waits up to `waitSeconds` (default 10) for the
   * solver's funding to appear before giving up for this pass; the claim
   * itself is bounded by the quote's `refund_locktime`.
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
    const secrets = this.secretsOf(record);
    const script = new VHTLC.ScriptV2(
      deserializeVhtlcOptions(record.scriptOptions),
    );
    const deadline = Math.min(
      record.quote.refund_locktime ?? Number.POSITIVE_INFINITY,
      this.now() + (options?.waitSeconds ?? 10),
    );
    const { arkTxid } = await this.flows.claimReceiveLockup(
      this.indexer,
      this.ark,
      {
        swapPkScript: hex.decode(record.swapPkScriptHex),
        // Required by the type but ignored by the implementation, which waits
        // for the funding and substitutes what it finds.
        vtxos: [],
        script,
        receiver: await senderIdentityForRfqSecrets(this.wallet, secrets),
        preimage: await preimageForRfqSecrets(this.wallet, secrets),
        destinationPkScript: ArkAddress.decode(this.payoutAddressOf(record))
          .pkScript,
        expectedAmount: this.expectedAmountOf(record),
        deadline,
      },
    );
    record.phase = "settled";
    record.resolvedTxid = arkTxid;
    await this.store.put(record);
    return record;
  }

  /**
   * Resolve a stalled send: returns as soon as the solver settled or
   * refunded, otherwise pushes the trader's `refundWithoutReceiver` once
   * `refund_locktime` has matured. Safe to call repeatedly and late.
   */
  async refundSend(id: string): Promise<ArkadeSwapRecord> {
    const record = await this.mustGet(id);
    if (record.route !== "arkade:BTC->lightning:BTC") {
      throw new Error(`refundSend: ${id} is not a send swap`);
    }
    const secrets = this.secretsOf(record);
    const script = new VHTLC.ScriptV2(
      deserializeVhtlcOptions(record.scriptOptions),
    );
    const outcome = await this.flows.refundIfUnresolved(
      this.transport,
      this.ark,
      this.indexer,
      {
        rfqId: record.id,
        script,
        sender: await senderIdentityForRfqSecrets(this.wallet, secrets),
        refundLocktime: this.refundLocktimeOf(record),
        now: this.now,
      },
    );
    switch (outcome.outcome) {
      case "resolved": {
        record.phase =
          outcome.status.state === "refunded" ? "refunded" : "settled";
        // The solver's terminal receipt may name the settling transaction;
        // best-effort, since the profile shape is solver-defined.
        const txid = outcome.status.profile?.txid;
        if (typeof txid === "string") record.resolvedTxid = txid;
        break;
      }
      case "refunded":
        record.phase = "refunded";
        record.resolvedTxid = outcome.arkTxid;
        break;
      case "nothing_to_refund": {
        // The lockup is empty. Chain evidence — never the local record —
        // says which way it went: a preimage-revealing spend is the solver
        // completing (invoice paid), any other spend returned the funds,
        // and a lockup that never saw an output means the quote simply
        // lapsed. `fundingTxid` is NOT consulted: `notifyFunded(id)` may
        // legitimately have recorded no txid, and the host may have
        // funded and crashed before calling it at all.
        const fate = await this.fateOf(record);
        record.phase =
          fate.fate === "claimed"
            ? "settled"
            : fate.fate === "returned"
              ? "refunded"
              : fate.fate === "unknown"
                ? "cancelled"
                : record.phase; // "open" contradicts nothing_to_refund; re-read next pass
        break;
      }
      case "needs_recovery":
        record.phase = "needs_recovery";
        record.recoveryOutpoints = outcome.outpoints;
        break;
    }
    await this.store.put(record);
    return record;
  }

  /** In-flight reconcile pass; a second caller joins it instead of racing. */
  private reconcilePass: Promise<ReconcileReport> | null = null;

  /**
   * One evidence-driven pass over every pending record. Never throws for a
   * single record's failure — errors are reported and the record keeps its
   * phase for the next pass. Designed to be called from a host alarm/timer;
   * a relay timeout inside any step means "unknown", not "failed".
   *
   * Re-entrant calls share the running pass: a pass can spend seconds
   * waiting on a claim, and a host alarm firing meanwhile must not run a
   * second pass over the same records.
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
      pending: [],
      errors: [],
    };
    const file = (record: ArkadeSwapRecord) => {
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
        default:
          report.pending.push(record.id);
      }
    };
    for (const record of await this.store.listPending()) {
      try {
        file(
          record.route === "arkade:BTC->lightning:BTC"
            ? await this.reconcileSend(record)
            : await this.reconcileReceive(record),
        );
      } catch (error) {
        report.errors.push({ id: record.id, error });
      }
    }
    if (this.assetSwaps) await this.reconcileAssetSwaps(report);
    return report;
  }

  private async reconcileSend(
    record: ArkadeSwapRecord,
  ): Promise<ArkadeSwapRecord> {
    if (record.phase === "prepared" && this.now() < record.quote.valid_until) {
      // Inside the commitment window with no funding reported: nothing to
      // do yet, and no reason to poll the chain every pass.
      return record;
    }
    // Everything else is decided from chain evidence first. In particular a
    // `prepared` record past `valid_until` is NOT cancelled on the local
    // flag alone: funding is acceptance, so the host may have broadcast the
    // lockup and died before `notifyFunded` — a record dropped from the
    // pending set here would strand real sats in a VHTLC whose derivation
    // only this record holds.
    const fate = await this.fateOf(record);
    if (fate.fate === "claimed") {
      record.phase = "settled";
      await this.store.put(record);
      return record;
    }
    if (fate.fate === "returned") {
      record.phase = "refunded";
      await this.store.put(record);
      return record;
    }
    if (fate.fate === "unknown") {
      if (record.phase === "prepared") {
        // Past the window and the chain has never seen the lockup: the
        // negotiation is dead and nothing was ever at stake.
        record.phase = "cancelled";
        await this.store.put(record);
      }
      // `funded` + no chain trace: the funding may still be propagating —
      // keep watching rather than invent a terminal state.
      return record;
    }
    // The lockup is live. A prepared record self-heals to funded (the
    // crash-between-broadcast-and-notify case), and a matured one refunds.
    if (record.phase === "prepared") {
      record.phase = "funded";
      await this.store.put(record);
    }
    if (this.now() >= this.refundLocktimeOf(record)) {
      return this.refundSend(record.id);
    }
    return record;
  }

  private async reconcileReceive(
    record: ArkadeSwapRecord,
  ): Promise<ArkadeSwapRecord> {
    const deadline = record.quote.refund_locktime;
    if (record.phase === "prepared") {
      // The invoice was never reported paid. Once it can no longer be paid,
      // nothing can arm the swap.
      const expiry = record.invoiceExpiresAt ?? record.quote.valid_until;
      if (this.now() >= expiry) {
        record.phase = "cancelled";
        await this.store.put(record);
      }
      return record;
    }
    // Payment dispatched: claim as soon as the solver's lockup appears. Past
    // the solver's refund horizon the claim window is closed — claiming
    // would publish the preimage into a refund race — but the record is NOT
    // cancelled on the clock alone: `covclaimdPubkey` exists precisely so
    // the solver's claim daemon can claim for us while we were offline, and
    // that claim settled the swap.
    if (deadline !== undefined && this.now() >= deadline) {
      const fate = await this.fateOf(record);
      if (fate.fate === "claimed") {
        record.phase = "settled";
      } else if (fate.fate === "returned") {
        // The solver reclaimed its lockup; our LN payment fails back.
        record.phase = "refunded";
      } else if (fate.fate === "unknown") {
        // Never funded and no longer claimable.
        record.phase = "cancelled";
      } else {
        // Still open past the deadline: the solver's to resolve — claiming
        // now would race its refund. Keep watching.
        return record;
      }
      await this.store.put(record);
      return record;
    }
    try {
      return await this.claimReceive(record.id, { waitSeconds: 5 });
    } catch {
      // Not funded yet (or a transient push failure): unknown, not failed.
      return record;
    }
  }

  private async mustGet(id: string): Promise<ArkadeSwapRecord> {
    const record = await this.store.get(id);
    if (!record) throw new Error(`unknown swap record: ${id}`);
    return record;
  }

  private secretsOf(record: ArkadeSwapRecord): SwapSecrets {
    const secrets = rfqSecretsOfRecord(record.secrets);
    if (!secrets)
      throw new Error(`swap ${record.id}: secrets record is unusable`);
    return secrets;
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

  private async fateOf(record: ArkadeSwapRecord): Promise<LockupFate> {
    const paymentHash = record.quote.profile.payment_hash;
    if (typeof paymentHash !== "string") {
      throw new Error(`swap ${record.id}: quote profile has no payment_hash`);
    }
    return this.flows.readLockupFate(this.indexer, {
      swapPkScript: hex.decode(record.swapPkScriptHex),
      paymentHash,
    });
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
    const offer = await this.flows.createOffer(
      this.wallet,
      this.arkServerUrl,
      {
        wantAmount: params.wantAmountAtomic,
        wantAsset: params.wantAssetId
          ? asset.AssetId.fromString(params.wantAssetId)
          : undefined,
        offerAsset: params.offerAssetId
          ? asset.AssetId.fromString(params.offerAssetId)
          : undefined,
        emulatorPubkey: params.emulatorPubkey,
      },
    );
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
