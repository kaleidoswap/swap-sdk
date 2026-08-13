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
 * crossing this boundary must come from that same SDK line (>= 0.4.60 — the
 * `VHTLC.ScriptV2` era). The host app owns the pin.
 */

import type { IWallet } from "@arkade-os/sdk";
import {
  ArkAddress,
  RestArkProvider,
  RestIndexerProvider,
  VHTLC,
} from "@arkade-os/sdk";
import type {
  InvoiceFacts,
  LockupFate,
  RefundArkProvider,
  RefundIndexer,
  RfqQuote,
  RfqTransport,
  SwapSecrets,
} from "@arkade-os/swap";
import {
  claimReceiveLockup,
  preimageForRfqSecrets,
  readLockupFate,
  refundIfUnresolved,
  requestLightningReceive,
  requestLightningSend,
  rfqSecretsOfRecord,
  rfqSecretsToRecord,
  senderIdentityForRfqSecrets,
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

/** What one `reconcile()` pass did, keyed by record id. */
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
}

export interface ArkadeIntentsVenueOptions {
  wallet: IWallet;
  arkServerUrl: string;
  /** RFQ transport from the solver's card (`nostrRfqTransport`, HTTP, …). */
  transport: RfqTransport;
  store: ArkadeSwapStore;
  /** Defaults to REST providers on `arkServerUrl`. */
  arkProvider?: RefundArkProvider;
  indexerProvider?: RefundIndexer & Parameters<typeof readLockupFate>[0];
  /** Unix seconds; injectable for tests. */
  now?: () => number;
  flows?: Partial<ArkadeIntentsFlows>;
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

  constructor(options: ArkadeIntentsVenueOptions) {
    this.wallet = options.wallet;
    this.arkServerUrl = options.arkServerUrl;
    this.transport = options.transport;
    this.store = options.store;
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
   */
  async notifyFunded(
    id: string,
    fundingTxid?: string,
  ): Promise<ArkadeSwapRecord> {
    const record = await this.mustGet(id);
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
      case "resolved":
        record.phase =
          outcome.status.state === "refunded" ? "refunded" : "settled";
        break;
      case "refunded":
        record.phase = "refunded";
        record.resolvedTxid = outcome.arkTxid;
        break;
      case "nothing_to_refund": {
        // The lockup is empty. Chain evidence says which way it went:
        // a preimage-revealing spend is the solver completing (invoice
        // paid); anything else returned the funds; never funded = the
        // quote simply lapsed.
        if (!record.fundingTxid) {
          record.phase = "cancelled";
          break;
        }
        const fate = await this.fateOf(record);
        record.phase = fate.fate === "claimed" ? "settled" : "refunded";
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

  /**
   * One evidence-driven pass over every pending record. Never throws for a
   * single record's failure — errors are reported and the record keeps its
   * phase for the next pass. Designed to be called from a host alarm/timer;
   * a relay timeout inside any step means "unknown", not "failed".
   */
  async reconcile(): Promise<ReconcileReport> {
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
    return report;
  }

  private async reconcileSend(
    record: ArkadeSwapRecord,
  ): Promise<ArkadeSwapRecord> {
    if (record.phase === "prepared") {
      // Nothing funded. Past the quote's commitment window the negotiation
      // is dead — nothing was ever at stake.
      if (this.now() >= record.quote.valid_until) {
        record.phase = "cancelled";
        await this.store.put(record);
      }
      return record;
    }
    // Funded: read the lockup's fate before considering a refund push.
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
    // would publish the preimage into a refund race.
    if (deadline !== undefined && this.now() >= deadline) {
      record.phase = "cancelled";
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
    const locktime = record.quote.refund_locktime;
    if (locktime === undefined)
      throw new Error(`swap ${record.id}: quote has no refund_locktime`);
    return locktime;
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
}
