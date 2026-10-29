// KaleidoSwap — the high-level surface.
//
// One verb per step — `quote`, `start`, `resume` — over the routes the maker
// serves. The protocol a quote rides on is reported, never chosen by the
// caller. Everything the core does stays reachable from the main entry; this
// layer is built on that public surface and holds no key material of its own:
// keys come from the injected `SwapSigner`, and every byte a swap needs to
// finish or unwind after a restart goes through the injected `SwapStore`
// before the irreversible step it precedes.
//
// This file takes the wasm-backed core as a parameter (`makeCreateKaleidoSwap`)
// so it imports nothing with side effects and the Node and browser entries can
// each bind their own `init`.

import type {
  DerivedKey,
  DerivedPreimage,
  Network,
  SwapClient,
  SwapScript,
  TxParams,
} from "./index.js";
import { decodeBolt11 } from "./pay-through.js";

// ---------------------------------------------------------------------------
// Assets and amounts

/** The chain an asset lives on. `arkade` is Bitcoin on Arkade (`ARKD`). */
export type Chain = "bitcoin" | "lightning" | "liquid" | "arkade";

/**
 * `<chain>:<asset>`. Spelling the chain is what makes a wrong-network
 * destination a local error instead of an unrecoverable payout.
 */
export type AssetId =
  "bitcoin:BTC" | "lightning:BTC" | "liquid:BTC" | "liquid:USDT" | "arkade:BTC";

/**
 * Atomic units of the asset — sats for BTC on any chain, L-USDT units for
 * `liquid:USDT`. A `number` must be a safe integer; a `string` is parsed as
 * one. Everything is `bigint` past this boundary.
 */
export type Amount = bigint | number | string;

// ---------------------------------------------------------------------------
// Injected dependencies

/**
 * Where the SDK keeps everything a swap needs to finish or unwind after a
 * restart: the swap index, the maker's response, `swapAuth`, and the last
 * known phase. Records are plain JSON — no `bigint`, no class instances — so
 * persisting them verbatim is the whole job. Every write lands before the
 * irreversible step it precedes.
 */
export interface SwapStore {
  get(id: string): Promise<SwapRecord | undefined>;
  put(record: SwapRecord): Promise<void>;
  list(): Promise<SwapRecord[]>;
  /**
   * Hands out the next BIP85 child index, exactly once. An index reused is
   * claim and refund material reused, so this has to be atomic for the store.
   */
  nextIndex(): Promise<bigint>;
}

/**
 * Derives per-swap material from a root the caller owns. `mnemonicSigner`
 * wraps `SwapMasterKey`; a wallet that keeps its seed elsewhere implements
 * this instead. The SDK never sees a mnemonic through it.
 */
export interface SwapSigner {
  deriveKey(index: bigint): Promise<DerivedKey>;
  derivePreimage(index: bigint): Promise<DerivedPreimage>;
}

/**
 * Chain access for the network the instance was created on. Resolved per
 * `Network`, so a signet maker can never be paired with testnet3 chain access
 * — the mix the README warns fails silently.
 */
export interface ChainAccess {
  bitcoinEsploraUrl: string;
  liquidEsploraUrl: string;
}

// ---------------------------------------------------------------------------
// Construction

export interface KaleidoSwapOptions {
  network: Network;
  store: SwapStore;
  signer: SwapSigner;
  /** Defaults to the KaleidoSwap endpoints for `network`; required where there are none. */
  chain?: Partial<ChainAccess>;
  /** The maker's `/v2` base URL. Defaults per network exactly as `SwapClient.forNetwork` does. */
  makerUrl?: string;
  /** How often a swap in flight is asked for its status. Default 2 s. */
  pollIntervalMs?: number;
  /** Fee rate for claims and refunds this layer broadcasts. Default 2 sat/vB. */
  feeSatPerVb?: number;
  /**
   * How long a quote may be held before `start` refuses it. Pair terms are
   * bound by hash, so this guards staleness, not price. Default 10 minutes.
   */
  quoteTtlMs?: number;
}

// ---------------------------------------------------------------------------
// Quote

export interface QuoteRequest {
  from: AssetId;
  to: AssetId;
  /** Which side `amount` fixes. Defaults to `"from"`. */
  side?: "from" | "to";
  /** Omit when `invoice` fixes it. */
  amount?: Amount;
  /**
   * Required when `to` is Lightning: the route is priced against the invoice
   * the maker will pay, and its amount is the `to` amount.
   */
  invoice?: string;
}

/** The wire a quote rides on. Informational; the caller never picks one. */
export type Protocol =
  "boltz-submarine" | "boltz-reverse" | "arkade-rfq" | "pay-through";

export interface Quote {
  kind: "quote";
  protocol: Protocol;
  from: AssetId;
  to: AssetId;
  fromAmount: bigint;
  toAmount: bigint;
  fees: { maker: bigint; miner: bigint };
  limits: { min: bigint; max: bigint };
  /** Accepting after this is a new quote, not this one. */
  expiresAt: Date;
  /** What holds the maker to these terms. Opaque. */
  readonly binding: unknown;
}

/**
 * The maker's answer when it will not trade. A value, not an error, exactly
 * as `RfqAnswer` already does for the Arkade corridor.
 */
export interface Refusal {
  kind: "refusal";
  from: AssetId;
  to: AssetId;
  reason:
    | "pair_unavailable"
    | "amount_below_min"
    | "amount_above_max"
    | "no_liquidity"
    | "maker_paused";
  limits?: { min: bigint; max: bigint };
  message: string;
}

export type QuoteAnswer = Quote | Refusal;

export function isQuote(answer: QuoteAnswer): answer is Quote {
  return answer.kind === "quote";
}

// ---------------------------------------------------------------------------
// Start

export interface StartOptions {
  /**
   * Where the `to` asset lands: an address on its chain. Validated against
   * `to` and the network before anything is sent. Omitted when the quote was
   * priced against an invoice — that invoice is the destination.
   */
  destination?: string;
  /**
   * Where a refund lands if the swap unwinds. Required when `from` is an
   * on-chain asset; ignored for Lightning, which refunds by not settling.
   */
  refundAddress?: string;
  signal?: AbortSignal;
}

// ---------------------------------------------------------------------------
// Swap lifecycle

/**
 * One state machine over every protocol. The core's per-protocol statuses
 * (`transaction.confirmed`, `invoice.settled`, …) map onto these; the raw one
 * stays readable on {@link SwapRecord.rawStatus}.
 *
 *   created ─▶ awaiting_funding ─▶ funded ─▶ locked ─▶ claiming ─▶ settled
 *                                              │
 *                                              ├─▶ refundable ─▶ refunding ─▶ refunded
 *                                              └─▶ failed
 */
export type SwapPhase =
  | "created"
  | "awaiting_funding"
  | "funded"
  | "locked"
  | "claiming"
  | "settled"
  | "refundable"
  | "refunding"
  | "refunded"
  | "failed";

export const TERMINAL_PHASES: ReadonlySet<SwapPhase> = new Set<SwapPhase>([
  "settled",
  "refunded",
  "failed",
]);

/** What the store holds. Opaque to the caller beyond the fields named here. */
export interface SwapRecord {
  id: string;
  protocol: Protocol;
  network: Network;
  makerUrl: string;
  from: AssetId;
  to: AssetId;
  phase: SwapPhase;
  /** The protocol's own status string, for logs and support. */
  rawStatus: string;
  createdAt: string;
  updatedAt: string;
  /** Swap index, maker response, swapAuth, destination. Never logged. */
  readonly state: unknown;
}

/**
 * What the caller acts on. Only `invoice` and `fundingAddress` are ever
 * needed; the rest is for showing the user where things stand.
 */
export interface SwapAction {
  /** Pay this to fund a swap from Lightning. */
  invoice?: string;
  /** Send exactly `fundingAmount` here to fund a swap from an on-chain asset. */
  fundingAddress?: string;
  fundingAmount?: bigint;
  /** Zero means the maker waits for a confirmation before acting. */
  zeroConfLimit?: bigint;
}

export interface Swap {
  readonly id: string;
  readonly protocol: Protocol;
  readonly phase: SwapPhase;
  readonly action: SwapAction;

  /** Phase transitions as they happen; ends on a terminal phase. */
  phases(signal?: AbortSignal): AsyncIterable<SwapPhase>;
  /** Resolves on `settled`, rejects with a {@link SwapError} on any other terminal phase. */
  settled(signal?: AbortSignal): Promise<SwapRecord>;
  /**
   * Unwinds a `refundable` swap cooperatively, or after the timeout if the
   * maker will not co-sign. The driver calls this on its own; it is here for
   * a caller who wants to decide.
   */
  refund(signal?: AbortSignal): Promise<SwapRecord>;
  /** The persisted record, as the store holds it right now. */
  record(): Promise<SwapRecord>;
}

// ---------------------------------------------------------------------------
// Errors

export type SwapErrorCode =
  | "INVALID_AMOUNT"
  | "AMOUNT_OUT_OF_LIMITS"
  | "PAIR_UNAVAILABLE"
  | "QUOTE_EXPIRED"
  | "WRONG_NETWORK_ADDRESS"
  | "INVALID_DESTINATION"
  | "REFUND_ADDRESS_REQUIRED"
  | "LOCKUP_MISMATCH"
  | "MAKER_UNREACHABLE"
  | "MAKER_REJECTED"
  | "SWAP_EXPIRED"
  | "SWAP_FAILED"
  | "SWAP_NOT_FOUND"
  | "STORE_FAILED"
  | "ABORTED";

/** Every failure crosses this boundary coded; the core's uncoded throws are mapped here. */
export class SwapError extends Error {
  constructor(
    readonly code: SwapErrorCode,
    message: string,
    readonly swapId?: string,
    options?: { cause?: unknown },
  ) {
    super(message);
    this.name = "SwapError";
    if (options?.cause !== undefined) this.cause = options.cause;
  }
  cause?: unknown;
}

export function isSwapError(error: unknown): error is SwapError {
  return error instanceof SwapError;
}

// ---------------------------------------------------------------------------
// The instance

export interface ResumeReport {
  /** Still in flight after one pass; follow each with `settled()` or `phases()`. */
  resumed: Swap[];
  settled: SwapRecord[];
  refunded: SwapRecord[];
  failed: SwapRecord[];
}

export interface KaleidoSwap {
  readonly network: Network;

  quote(request: QuoteRequest): Promise<QuoteAnswer>;

  /**
   * Creates the swap against `quote`, verifies the lockup against our own
   * key, persists the record, then returns. Nothing is funded or paid here —
   * that is the caller's step, via {@link Swap.action}.
   */
  start(quote: Quote, options?: StartOptions): Promise<Swap>;

  /** A swap this instance or an earlier one started, by id. */
  swap(id: string): Promise<Swap>;

  /**
   * Picks every non-terminal record out of the store, asks the maker where
   * each stands, and moves it one step: claims what is claimable, refunds
   * what is refundable, reports the rest. Call it at startup, before
   * anything else. It does not wait for funding or confirmations — the
   * `resumed` handles do, when followed.
   */
  resume(signal?: AbortSignal): Promise<ResumeReport>;
}

// ---------------------------------------------------------------------------
// In-memory store — tests and throwaway scripts. Nothing survives the process.

export class MemoryStore implements SwapStore {
  private readonly records = new Map<string, SwapRecord>();
  private index = 0n;

  constructor(startIndex: bigint = 0n) {
    this.index = startIndex;
  }

  async get(id: string): Promise<SwapRecord | undefined> {
    return this.records.get(id);
  }
  async put(record: SwapRecord): Promise<void> {
    this.records.set(record.id, record);
  }
  async list(): Promise<SwapRecord[]> {
    return [...this.records.values()];
  }
  async nextIndex(): Promise<bigint> {
    return this.index++;
  }
}

// ---------------------------------------------------------------------------
// Internals

/** The core this layer is built on, handed in so each entry binds its own `init`. */
export interface CoreDeps {
  init(): Promise<void>;
  SwapClient: {
    forNetwork(network: string): SwapClient;
    new (baseUrl: string, timeoutSecs?: bigint | null): SwapClient;
  };
  SwapScript: {
    fromSubmarine(
      chainKind: "bitcoin" | "liquid",
      network: Network,
      response: unknown,
      ourPubkeyHex: string,
    ): SwapScript;
    fromReverse(
      chainKind: "bitcoin" | "liquid",
      network: Network,
      response: unknown,
      ourPubkeyHex: string,
    ): SwapScript;
  };
}

/**
 * The on-chain steps, as a seam. The default builds and broadcasts through
 * `SwapScript`; tests replace it so nothing here needs a chain.
 */
export interface SwapFlows {
  verifyLockup(record: SwapRecord, ourPubkey: string): void;
  claim(record: SwapRecord, key: DerivedKey, preimage: string): Promise<string>;
  refund(record: SwapRecord, key: DerivedKey): Promise<string>;
}

interface BoltzState {
  swapIndex: string;
  response: Record<string, unknown>;
  /** Dot paths of the `response` fields that crossed the wasm boundary as `bigint`. */
  bigints?: string[];
  destination: string;
  refundAddress?: string;
  claimTxid?: string;
  refundTxid?: string;
  /** Whether a lockup from us was ever seen — decides `failed` vs `refundable` on expiry. */
  fundedSeen?: boolean;
}

interface QuoteBinding {
  pairHash: string;
  invoice?: string;
  zeroConfLimit: bigint;
}

const DEFAULT_MAKER: Partial<Record<Network, string>> = {
  signet: "https://maker.signet.kaleidoswap.com/v2",
  regtest: "http://localhost:9001/v2",
};

const DEFAULT_CHAIN: Partial<Record<Network, ChainAccess>> = {
  signet: {
    bitcoinEsploraUrl: "https://esplora.signet.kaleidoswap.com",
    // Blockstream's liquidtestnet Esplora rate-limits UTXO fetches.
    liquidEsploraUrl: "https://liquid.network/liquidtestnet/api",
  },
  regtest: {
    bitcoinEsploraUrl: "http://localhost:3002",
    liquidEsploraUrl: "http://localhost:3003",
  },
};

function chainOf(asset: AssetId): Chain {
  return asset.split(":")[0] as Chain;
}

/** The maker's catalogue symbol for an asset. */
function symbolOf(asset: AssetId): string {
  switch (asset) {
    case "bitcoin:BTC":
    case "lightning:BTC":
      return "BTC";
    case "liquid:BTC":
      return "L-BTC";
    case "liquid:USDT":
      return "L-USDT";
    case "arkade:BTC":
      return "ARKD";
  }
}

function toBigint(value: Amount, what: string): bigint {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value))
      throw new SwapError(
        "INVALID_AMOUNT",
        `${what} must be a safe integer in atomic units, got ${value}`,
      );
    return BigInt(value);
  }
  if (/^\d+$/.test(value)) return BigInt(value);
  throw new SwapError("INVALID_AMOUNT", `${what} is not an integer: ${value}`);
}

/** `ceil(amount * percent / 100)`, percent as the maker publishes it (e.g. 0.5). */
function percentFee(amount: bigint, percent: number): bigint {
  const basisPoints = BigInt(Math.round(percent * 10_000));
  const scaled = amount * basisPoints;
  return (scaled + 999_999n) / 1_000_000n;
}

function minerFeeTotal(fees: Record<string, unknown>): bigint {
  const miner = fees.minerFees;
  if (typeof miner === "bigint") return miner;
  if (typeof miner === "object" && miner !== null) {
    const parts = miner as Record<string, bigint>;
    return (parts.lockup ?? 0n) + (parts.claim ?? 0n);
  }
  return 0n;
}

const ADDRESS_PREFIXES: Record<
  Exclude<Chain, "arkade">,
  Partial<Record<Network, string[]>>
> = {
  bitcoin: {
    mainnet: ["bc1", "1", "3"],
    testnet: ["tb1", "m", "n", "2"],
    signet: ["tb1", "m", "n", "2"],
    regtest: ["bcrt1"],
  },
  liquid: {
    mainnet: ["lq1", "ex1", "VJL", "VTp"],
    testnet: ["tlq1", "tex1", "vjT", "vjU"],
    signet: ["tlq1", "tex1", "vjT", "vjU"],
    regtest: ["el1", "ert1", "CTE", "AzP"],
  },
  lightning: {
    mainnet: ["lnbc"],
    testnet: ["lntb"],
    signet: ["lntbs"],
    regtest: ["lnbcrt"],
  },
};

/** A prefix check, not validation — it catches a mainnet address used on signet. */
function checkAddress(
  value: string,
  asset: AssetId,
  network: Network,
  what: string,
): void {
  const chain = chainOf(asset);
  if (chain === "arkade") return;
  const prefixes = ADDRESS_PREFIXES[chain][network] ?? [];
  const matches = prefixes.some((prefix) =>
    chain === "lightning"
      ? value.toLowerCase().startsWith(prefix) &&
        // `lntb` is testnet3; `lntbs` is signet. Keep them apart.
        (prefix !== "lntb" || !value.toLowerCase().startsWith("lntbs"))
      : value.startsWith(prefix),
  );
  if (!matches)
    throw new SwapError(
      "WRONG_NETWORK_ADDRESS",
      `${what} ${value} does not look like a ${network} ${chain} ${
        chain === "lightning" ? "invoice" : "address"
      } (expected a prefix among ${prefixes.join(", ")})`,
    );
}

/** JSON-plain copy: `bigint` becomes a decimal string so any store can hold it. */
/**
 * JSON-plain copy: `bigint` becomes a decimal string so any store can hold
 * it, and the paths that held one are returned so {@link revive} can put them
 * back before the object crosses into the wasm again — which deserializes a
 * `u64` from a number or a `bigint`, never from a string.
 */
function plain<T>(value: T): { value: T; bigints: string[] } {
  const bigints: string[] = [];
  const walk = (v: unknown, path: string): unknown => {
    if (typeof v === "bigint") {
      bigints.push(path);
      return v.toString();
    }
    if (Array.isArray(v)) return v.map((x, i) => walk(x, `${path}[${i}]`));
    if (v && typeof v === "object")
      return Object.fromEntries(
        Object.entries(v as Record<string, unknown>).map(([k, x]) => [
          k,
          walk(x, path ? `${path}.${k}` : k),
        ]),
      );
    return v;
  };
  return { value: JSON.parse(JSON.stringify(walk(value, ""))) as T, bigints };
}

/** The create responses' 64-bit fields, for records written before `bigints` was kept. */
const KNOWN_BIGINT_FIELDS = [
  "onchainAmount",
  "expectedAmount",
  "timeoutBlockHeight",
  "invoiceAmount",
];

function revive(
  response: Record<string, unknown>,
  bigints: string[] | undefined,
): Record<string, unknown> {
  const out = JSON.parse(JSON.stringify(response)) as Record<string, unknown>;
  for (const path of bigints ?? KNOWN_BIGINT_FIELDS) {
    const keys = path.split(/\.|\[|\]/).filter(Boolean);
    let cursor: unknown = out;
    for (const key of keys.slice(0, -1))
      cursor = (cursor as Record<string, unknown> | undefined)?.[key];
    const last = keys[keys.length - 1];
    const v = (cursor as Record<string, unknown> | undefined)?.[last];
    if (typeof v === "string" && /^\d+$/.test(v))
      (cursor as Record<string, unknown>)[last] = BigInt(v);
  }
  return out;
}

function asBigint(value: unknown): bigint {
  return typeof value === "bigint" ? value : BigInt(String(value));
}

function fromCore(error: unknown, swapId?: string): SwapError {
  if (error instanceof SwapError) return error;
  const message = error instanceof Error ? error.message : String(error);
  const code = (error as { code?: unknown })?.code;
  if (code === "HTTP") {
    const rejected = /\b4\d\d\b/.test(message);
    return new SwapError(
      rejected ? "MAKER_REJECTED" : "MAKER_UNREACHABLE",
      message,
      swapId,
      { cause: error },
    );
  }
  return new SwapError("SWAP_FAILED", message, swapId, { cause: error });
}

function throwIfAborted(signal?: AbortSignal, swapId?: string): void {
  if (signal?.aborted)
    throw new SwapError("ABORTED", "the operation was aborted", swapId);
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(done, ms);
    function done() {
      signal?.removeEventListener("abort", done);
      clearTimeout(timer);
      resolve();
    }
    signal?.addEventListener("abort", done, { once: true });
  });
}

// ---------------------------------------------------------------------------
// Phase mapping

type Transition = { phase: SwapPhase; fundedSeen?: true };

/**
 * Boltz statuses onto the shared machine. A submarine that expires or whose
 * invoice fails after we funded it is refundable; before funding there is
 * nothing to refund. A reverse swap never needs a refund from us — the maker's
 * hold invoice expires on its own — so every failure there is `failed`.
 */
function mapStatus(
  protocol: Protocol,
  status: string,
  state: BoltzState,
): Transition {
  const refundable: SwapPhase =
    protocol === "boltz-submarine" && state.fundedSeen
      ? "refundable"
      : "failed";
  switch (status) {
    case "swap.created":
    case "invoice.set":
      return { phase: "awaiting_funding" };
    case "minerfee.paid":
      return { phase: "awaiting_funding" };
    case "transaction.mempool":
    case "transaction.zeroconf.rejected":
      return { phase: "funded", fundedSeen: true };
    case "transaction.confirmed":
      return { phase: "locked", fundedSeen: true };
    case "invoice.pending":
      return { phase: "locked", fundedSeen: true };
    case "transaction.claim.pending":
      return { phase: "claiming", fundedSeen: true };
    case "invoice.paid":
    case "invoice.settled":
    case "transaction.claimed":
      return { phase: "settled" };
    case "transaction.refunded":
      return { phase: "refunded" };
    case "transaction.lockupFailed":
    case "invoice.failedToPay":
      return { phase: "refundable", fundedSeen: true };
    case "swap.expired":
    case "invoice.expired":
    case "transaction.failed":
    case "invoice.error":
      return { phase: refundable };
    default:
      return { phase: state.fundedSeen ? "funded" : "awaiting_funding" };
  }
}

// ---------------------------------------------------------------------------
// The implementation

interface Env {
  network: Network;
  makerUrl: string;
  chain: ChainAccess;
  client: SwapClient;
  store: SwapStore;
  signer: SwapSigner;
  flows: SwapFlows;
  pollIntervalMs: number;
  quoteTtlMs: number;
}

function defaultFlows(
  core: CoreDeps,
  env: Omit<Env, "flows">,
  feeSatPerVb: number,
): SwapFlows {
  const chainKind = (record: SwapRecord): "bitcoin" | "liquid" => {
    const onChain =
      record.protocol === "boltz-reverse" ? record.to : record.from;
    return chainOf(onChain) === "liquid" ? "liquid" : "bitcoin";
  };
  const script = (record: SwapRecord, pubkey: string): SwapScript => {
    const state = record.state as BoltzState;
    const response = revive(state.response, state.bigints);
    return record.protocol === "boltz-reverse"
      ? core.SwapScript.fromReverse(
          chainKind(record),
          record.network,
          response,
          pubkey,
        )
      : core.SwapScript.fromSubmarine(
          chainKind(record),
          record.network,
          response,
          pubkey,
        );
  };
  const params = (
    record: SwapRecord,
    key: DerivedKey,
    outputAddress: string,
  ): TxParams => ({
    outputAddress,
    swapId: record.id,
    keysSecretHex: key.secretKey,
    makerBaseUrl: record.makerUrl,
    network: record.network,
    bitcoinEsploraUrl: env.chain.bitcoinEsploraUrl,
    liquidEsploraUrl: env.chain.liquidEsploraUrl,
    feeSatPerVb,
  });
  const broadcast = (
    tx: {
      broadcast(
        n: string,
        b?: string | null,
        l?: string | null,
      ): Promise<string>;
    },
    record: SwapRecord,
  ) =>
    tx.broadcast(
      record.network,
      env.chain.bitcoinEsploraUrl,
      env.chain.liquidEsploraUrl,
    );

  return {
    verifyLockup(record, ourPubkey) {
      script(record, ourPubkey).free();
    },
    async claim(record, key, preimage) {
      const state = record.state as BoltzState;
      const s = script(record, key.publicKey);
      const p = params(record, key, state.destination);
      let tx;
      try {
        tx = await s.constructClaim(preimage, p);
      } catch (error) {
        // The maker co-signs Bitcoin claims but not every Liquid lockup; the
        // script path is always available, so fall back to it rather than
        // leave a claimable lockup behind.
        if (!/cooperative/i.test(String((error as Error)?.message)))
          throw error;
        tx = await s.constructClaim(preimage, { ...p, cooperative: false });
      }
      return broadcast(tx, record);
    },
    async refund(record, key) {
      const state = record.state as BoltzState;
      if (!state.refundAddress)
        throw new SwapError(
          "REFUND_ADDRESS_REQUIRED",
          "no refund address was recorded for this swap",
          record.id,
        );
      const tx = await script(record, key.publicKey).constructRefund(
        params(record, key, state.refundAddress),
      );
      return broadcast(tx, record);
    },
  };
}

class SwapHandle implements Swap {
  private current: SwapRecord;
  private driver?: Promise<void>;
  private readonly listeners = new Set<(phase: SwapPhase | Error) => void>();

  constructor(
    private readonly env: Env,
    record: SwapRecord,
  ) {
    this.current = record;
  }

  get id(): string {
    return this.current.id;
  }
  get protocol(): Protocol {
    return this.current.protocol;
  }
  get phase(): SwapPhase {
    return this.current.phase;
  }
  get action(): SwapAction {
    const state = this.current.state as BoltzState;
    const r = state.response;
    if (this.current.protocol === "boltz-reverse")
      return { invoice: r.invoice as string };
    return {
      fundingAddress: r.address as string,
      fundingAmount: asBigint(r.expectedAmount),
      zeroConfLimit: r.acceptZeroConf ? undefined : 0n,
    };
  }

  async record(): Promise<SwapRecord> {
    return (await this.env.store.get(this.id)) ?? this.current;
  }

  private async persist(
    patch: Partial<SwapRecord> & { state?: Partial<BoltzState> },
  ): Promise<SwapRecord> {
    const { state: statePatch, ...rest } = patch;
    const next: SwapRecord = {
      ...this.current,
      ...rest,
      state: { ...(this.current.state as BoltzState), ...statePatch },
      updatedAt: new Date().toISOString(),
    };
    try {
      await this.env.store.put(next);
    } catch (error) {
      throw new SwapError(
        "STORE_FAILED",
        `could not persist swap ${this.id}`,
        this.id,
        { cause: error },
      );
    }
    const changed = next.phase !== this.current.phase;
    this.current = next;
    if (changed) for (const l of this.listeners) l(next.phase);
    return next;
  }

  private async keyAndPreimage(): Promise<{
    key: DerivedKey;
    preimage: DerivedPreimage;
  }> {
    const index = BigInt((this.current.state as BoltzState).swapIndex);
    const [key, preimage] = await Promise.all([
      this.env.signer.deriveKey(index),
      this.env.signer.derivePreimage(index),
    ]);
    return { key, preimage };
  }

  /**
   * One observation of the maker plus whatever action it calls for. Returns
   * once that action's broadcast is recorded; the maker's own progress past it
   * shows up on the next tick.
   */
  async tick(signal?: AbortSignal): Promise<SwapRecord> {
    throwIfAborted(signal, this.id);
    if (TERMINAL_PHASES.has(this.current.phase)) return this.current;

    let status: string;
    try {
      status = (await this.env.client.swap(this.id)).status as string;
    } catch (error) {
      throw fromCore(error, this.id);
    }
    const state = this.current.state as BoltzState;
    const { phase, fundedSeen } = mapStatus(
      this.current.protocol,
      status,
      state,
    );

    // A claim or refund we already broadcast is not undone by the maker still
    // reporting the phase that triggered it.
    if (state.claimTxid && (phase === "locked" || phase === "funded"))
      return this.persist({ rawStatus: status, phase: "claiming" });
    if (state.refundTxid && phase === "refundable")
      return this.persist({ rawStatus: status, phase: "refunding" });

    await this.persist({
      rawStatus: status,
      phase,
      state: fundedSeen ? { fundedSeen } : {},
    });

    if (phase === "locked" && this.current.protocol === "boltz-reverse")
      return this.claimNow();
    if (phase === "refundable") return this.refundNow();
    return this.current;
  }

  private async claimNow(): Promise<SwapRecord> {
    const { key, preimage } = await this.keyAndPreimage();
    let txid: string;
    try {
      txid = await this.env.flows.claim(this.current, key, preimage.preimage);
    } catch (error) {
      throw fromCore(error, this.id);
    }
    return this.persist({ phase: "claiming", state: { claimTxid: txid } });
  }

  private async refundNow(): Promise<SwapRecord> {
    const { key } = await this.keyAndPreimage();
    let txid: string;
    try {
      txid = await this.env.flows.refund(this.current, key);
    } catch (error) {
      throw fromCore(error, this.id);
    }
    return this.persist({ phase: "refunding", state: { refundTxid: txid } });
  }

  async refund(signal?: AbortSignal): Promise<SwapRecord> {
    throwIfAborted(signal, this.id);
    const state = this.current.state as BoltzState;
    if (state.refundTxid) return this.current;
    if (this.current.phase !== "refundable")
      throw new SwapError(
        "SWAP_FAILED",
        `swap ${this.id} is ${this.current.phase}, not refundable`,
        this.id,
      );
    return this.refundNow();
  }

  private ensureDriver(signal?: AbortSignal): Promise<void> {
    if (this.driver) return this.driver;
    this.driver = (async () => {
      try {
        while (!TERMINAL_PHASES.has(this.current.phase)) {
          await this.tick(signal);
          if (TERMINAL_PHASES.has(this.current.phase)) break;
          await sleep(this.env.pollIntervalMs, signal);
          throwIfAborted(signal, this.id);
        }
      } catch (error) {
        const failure = fromCore(error, this.id);
        for (const l of this.listeners) l(failure);
        throw failure;
      } finally {
        this.driver = undefined;
      }
    })();
    // Rejections reach every subscriber through the listener; an unobserved
    // driver must not also surface as an unhandled rejection.
    this.driver.catch(() => {});
    return this.driver;
  }

  async *phases(signal?: AbortSignal): AsyncIterable<SwapPhase> {
    const queue: (SwapPhase | Error)[] = [];
    let wake: (() => void) | undefined;
    const listener = (event: SwapPhase | Error) => {
      queue.push(event);
      wake?.();
    };
    this.listeners.add(listener);
    const driver = this.ensureDriver(signal);
    let finished = false;
    driver.then(
      () => {
        finished = true;
        wake?.();
      },
      () => {
        finished = true;
        wake?.();
      },
    );
    try {
      for (;;) {
        while (queue.length > 0) {
          const event = queue.shift()!;
          if (event instanceof Error) throw event;
          yield event;
          if (TERMINAL_PHASES.has(event)) return;
        }
        if (finished) return;
        await new Promise<void>((resolve) => {
          wake = resolve;
        });
        wake = undefined;
      }
    } finally {
      this.listeners.delete(listener);
    }
  }

  async settled(signal?: AbortSignal): Promise<SwapRecord> {
    for await (const phase of this.phases(signal)) {
      if (phase === "settled") return this.current;
      if (phase === "refunded")
        throw new SwapError(
          "SWAP_FAILED",
          `swap ${this.id} was refunded`,
          this.id,
        );
      if (phase === "failed")
        throw new SwapError(
          this.current.rawStatus.includes("expired")
            ? "SWAP_EXPIRED"
            : "SWAP_FAILED",
          `swap ${this.id} failed: ${this.current.rawStatus}`,
          this.id,
        );
    }
    if (this.current.phase === "settled") return this.current;
    throw new SwapError(
      "SWAP_FAILED",
      `swap ${this.id} ended ${this.current.phase}`,
      this.id,
    );
  }
}

class KaleidoSwapImpl implements KaleidoSwap {
  constructor(private readonly env: Env) {}

  get network(): Network {
    return this.env.network;
  }

  async quote(request: QuoteRequest): Promise<QuoteAnswer> {
    const { from, to } = request;
    const fromChain = chainOf(from);
    const toChain = chainOf(to);
    const refusal = (message: string): Refusal => ({
      kind: "refusal",
      from,
      to,
      reason: "pair_unavailable",
      message,
    });

    if (
      fromChain === "lightning" &&
      (toChain === "bitcoin" || toChain === "liquid") &&
      to !== "liquid:USDT"
    )
      return this.quoteReverse(request);
    if (
      (fromChain === "bitcoin" || fromChain === "liquid") &&
      from !== "liquid:USDT" &&
      toChain === "lightning"
    )
      return this.quoteSubmarine(request);
    if (fromChain === "arkade" || toChain === "arkade")
      return refusal(
        `${from} -> ${to} rides the Arkade RFQ corridor, which this layer does not route yet; use @kaleidorg/swap-sdk/arkade`,
      );
    if (from === "liquid:USDT" || to === "liquid:USDT")
      return refusal(
        `${from} -> ${to} needs a caller-funded Liquid spend, which this layer does not route yet; use SwapScript.prepareLiquidClaim`,
      );
    return refusal(`${from} -> ${to} is not a route the maker serves`);
  }

  private async pairs(
    kind: "reverse" | "submarine",
    from: AssetId,
    to: AssetId,
  ): Promise<Record<string, unknown> | undefined> {
    let catalogue: Record<string, Record<string, Record<string, unknown>>>;
    try {
      catalogue =
        kind === "reverse"
          ? await this.env.client.reversePairs()
          : await this.env.client.submarinePairs();
    } catch (error) {
      throw fromCore(error);
    }
    return catalogue?.[symbolOf(from)]?.[symbolOf(to)];
  }

  private async quoteReverse(request: QuoteRequest): Promise<QuoteAnswer> {
    const { from, to } = request;
    if (request.amount === undefined)
      throw new SwapError(
        "INVALID_AMOUNT",
        "amount is required for a Lightning-funded swap",
      );
    const card = await this.pairs("reverse", from, to);
    if (!card)
      return {
        kind: "refusal",
        from,
        to,
        reason: "pair_unavailable",
        message: `the maker does not advertise ${from} -> ${to}`,
      };

    const limits = card.limits as Record<string, bigint>;
    const fees = card.fees as Record<string, unknown>;
    const percent = fees.percentage as number;
    const miner = minerFeeTotal(fees);
    const amount = toBigint(request.amount, "amount");

    let fromAmount: bigint;
    if (request.side === "to") {
      // Invert `to = from - ceil(from * pct) - miner` closely enough; the maker's
      // response carries the exact payout and the record keeps that one.
      const gross = amount + miner;
      fromAmount =
        (gross * 1_000_000n + 999_999n) /
        (1_000_000n - BigInt(Math.round(percent * 10_000)));
    } else {
      fromAmount = amount;
    }
    const makerFee = percentFee(fromAmount, percent);
    const toAmount = fromAmount - makerFee - miner;

    const bounds = { min: limits.minimal, max: limits.maximal };
    if (fromAmount < bounds.min)
      return {
        kind: "refusal",
        from,
        to,
        reason: "amount_below_min",
        limits: bounds,
        message: `${fromAmount} is below the pair minimum ${bounds.min}`,
      };
    if (fromAmount > bounds.max)
      return {
        kind: "refusal",
        from,
        to,
        reason: "amount_above_max",
        limits: bounds,
        message: `${fromAmount} is above the pair maximum ${bounds.max}`,
      };

    const binding: QuoteBinding = {
      pairHash: card.hash as string,
      zeroConfLimit: 0n,
    };
    return {
      kind: "quote",
      protocol: "boltz-reverse",
      from,
      to,
      fromAmount,
      toAmount,
      fees: { maker: makerFee, miner },
      limits: bounds,
      expiresAt: new Date(Date.now() + this.env.quoteTtlMs),
      binding,
    };
  }

  private async quoteSubmarine(request: QuoteRequest): Promise<QuoteAnswer> {
    const { from, to } = request;
    if (!request.invoice)
      throw new SwapError(
        "INVALID_DESTINATION",
        "a swap to Lightning is priced against the invoice: pass it as `invoice`",
      );
    checkAddress(request.invoice, to, this.env.network, "invoice");
    let decoded: ReturnType<typeof decodeBolt11>;
    try {
      decoded = decodeBolt11(request.invoice);
    } catch (error) {
      throw new SwapError(
        "INVALID_DESTINATION",
        error instanceof Error ? error.message : String(error),
      );
    }
    if (decoded.amountMsat === null)
      throw new SwapError(
        "INVALID_AMOUNT",
        "the invoice carries no amount; a submarine swap needs one",
      );
    const toAmount = decoded.amountMsat / 1000n;
    if (
      request.amount !== undefined &&
      toBigint(request.amount, "amount") !== toAmount
    )
      throw new SwapError(
        "INVALID_AMOUNT",
        `amount ${String(request.amount)} does not match the invoice's ${toAmount} sat`,
      );

    const card = await this.pairs("submarine", from, to);
    if (!card)
      return {
        kind: "refusal",
        from,
        to,
        reason: "pair_unavailable",
        message: `the maker does not advertise ${from} -> ${to}`,
      };

    const limits = card.limits as Record<string, bigint>;
    const fees = card.fees as Record<string, unknown>;
    const percent = fees.percentage as number;
    const miner = minerFeeTotal(fees);
    const makerFee = percentFee(toAmount, percent);
    const fromAmount = toAmount + makerFee + miner;

    const bounds = { min: limits.minimal, max: limits.maximal };
    if (toAmount < bounds.min)
      return {
        kind: "refusal",
        from,
        to,
        reason: "amount_below_min",
        limits: bounds,
        message: `${toAmount} is below the pair minimum ${bounds.min}`,
      };
    if (toAmount > bounds.max)
      return {
        kind: "refusal",
        from,
        to,
        reason: "amount_above_max",
        limits: bounds,
        message: `${toAmount} is above the pair maximum ${bounds.max}`,
      };

    const binding: QuoteBinding = {
      pairHash: card.hash as string,
      invoice: request.invoice,
      zeroConfLimit: limits.maximalZeroConf ?? 0n,
    };
    return {
      kind: "quote",
      protocol: "boltz-submarine",
      from,
      to,
      fromAmount,
      toAmount,
      fees: { maker: makerFee, miner },
      limits: bounds,
      expiresAt: new Date(Date.now() + this.env.quoteTtlMs),
      binding,
    };
  }

  async start(quote: Quote, options: StartOptions = {}): Promise<Swap> {
    throwIfAborted(options.signal);
    if (quote.kind !== "quote")
      throw new SwapError(
        "PAIR_UNAVAILABLE",
        "start takes a Quote, not a Refusal",
      );
    if (Date.now() > quote.expiresAt.getTime())
      throw new SwapError(
        "QUOTE_EXPIRED",
        "the quote has expired; ask for a new one",
      );
    const binding = quote.binding as QuoteBinding;
    const reverse = quote.protocol === "boltz-reverse";

    const destination = binding.invoice ?? options.destination;
    if (!destination)
      throw new SwapError("INVALID_DESTINATION", "destination is required");
    if (
      binding.invoice &&
      options.destination &&
      options.destination !== binding.invoice
    )
      throw new SwapError(
        "INVALID_DESTINATION",
        "destination differs from the invoice the quote was priced against",
      );
    checkAddress(destination, quote.to, this.env.network, "destination");
    if (!reverse) {
      if (!options.refundAddress)
        throw new SwapError(
          "REFUND_ADDRESS_REQUIRED",
          `a swap from ${quote.from} needs a refundAddress on its chain`,
        );
      checkAddress(
        options.refundAddress,
        quote.from,
        this.env.network,
        "refundAddress",
      );
    }

    const swapIndex = await this.env.store.nextIndex();
    const [key, preimage] = await Promise.all([
      this.env.signer.deriveKey(swapIndex),
      this.env.signer.derivePreimage(swapIndex),
    ]);

    let response: Record<string, unknown>;
    try {
      response = reverse
        ? await this.env.client.createReverseSwap(this.env.network, {
            from: symbolOf(quote.from),
            to: symbolOf(quote.to),
            invoiceAmount: quote.fromAmount,
            preimageHash: preimage.sha256,
            claimPublicKey: key.publicKey,
            pairHash: binding.pairHash,
          })
        : await this.env.client.createSubmarineSwap(this.env.network, {
            from: symbolOf(quote.from),
            to: symbolOf(quote.to),
            invoice: destination,
            refundPublicKey: key.publicKey,
            pairHash: binding.pairHash,
          });
    } catch (error) {
      throw fromCore(error);
    }

    // The id names a store entry — a file, for FileStore — so a maker cannot
    // be allowed to pick anything but a plain token.
    if (
      typeof response.id !== "string" ||
      !/^[A-Za-z0-9_-]{1,128}$/.test(response.id)
    )
      throw new SwapError(
        "MAKER_REJECTED",
        `the maker returned an unusable swap id: ${String(response.id)}`,
      );

    const now = new Date().toISOString();
    const plainResponse = plain(response);
    const state: BoltzState = {
      swapIndex: swapIndex.toString(),
      response: plainResponse.value,
      bigints: plainResponse.bigints,
      destination,
      refundAddress: options.refundAddress,
    };
    const record: SwapRecord = {
      id: response.id as string,
      protocol: quote.protocol,
      network: this.env.network,
      makerUrl: this.env.makerUrl,
      from: quote.from,
      to: quote.to,
      phase: "created",
      rawStatus: "swap.created",
      createdAt: now,
      updatedAt: now,
      state,
    };

    // A tree we could not claim or refund has to fail here, before anything
    // is funded — not after. Checked on the record exactly as the store will
    // hold it, so a restart rebuilds the same tree.
    try {
      this.env.flows.verifyLockup(record, key.publicKey);
    } catch (error) {
      throw new SwapError(
        "LOCKUP_MISMATCH",
        `the maker's lockup does not match our key: ${error instanceof Error ? error.message : String(error)}`,
        record.id,
        { cause: error },
      );
    }

    // swapAuth rides in `response`, issued once. This write is the one that
    // keeps it.
    try {
      await this.env.store.put(record);
    } catch (error) {
      throw new SwapError(
        "STORE_FAILED",
        `could not persist swap ${record.id}`,
        record.id,
        { cause: error },
      );
    }
    const handle = new SwapHandle(this.env, record);
    await handle["persist"]({ phase: "awaiting_funding" });
    return handle;
  }

  async swap(id: string): Promise<Swap> {
    const record = await this.env.store.get(id);
    if (!record)
      throw new SwapError("SWAP_NOT_FOUND", `no swap ${id} in the store`, id);
    return new SwapHandle(this.env, record);
  }

  async resume(signal?: AbortSignal): Promise<ResumeReport> {
    const report: ResumeReport = {
      resumed: [],
      settled: [],
      refunded: [],
      failed: [],
    };
    for (const record of await this.env.store.list()) {
      if (TERMINAL_PHASES.has(record.phase)) continue;
      const handle = new SwapHandle(this.env, record);
      const after = await handle.tick(signal);
      if (after.phase === "settled") report.settled.push(after);
      else if (after.phase === "refunded") report.refunded.push(after);
      else if (after.phase === "failed") report.failed.push(after);
      else report.resumed.push(handle);
    }
    return report;
  }
}

/** Builds the constructor over a core; each entry point exports the result. */
export function makeCreateKaleidoSwap(
  core: CoreDeps,
): (options: KaleidoSwapOptions, flows?: SwapFlows) => Promise<KaleidoSwap> {
  return async (options, flows) => {
    await core.init();
    const { network } = options;
    const makerUrl = options.makerUrl ?? DEFAULT_MAKER[network];
    if (!makerUrl)
      throw new SwapError(
        "PAIR_UNAVAILABLE",
        `KaleidoSwap runs no maker on ${network}; pass makerUrl to name one`,
      );
    const defaults = DEFAULT_CHAIN[network];
    const chain: ChainAccess = {
      bitcoinEsploraUrl:
        options.chain?.bitcoinEsploraUrl ?? defaults?.bitcoinEsploraUrl ?? "",
      liquidEsploraUrl:
        options.chain?.liquidEsploraUrl ?? defaults?.liquidEsploraUrl ?? "",
    };
    if (!chain.bitcoinEsploraUrl || !chain.liquidEsploraUrl)
      throw new SwapError(
        "PAIR_UNAVAILABLE",
        `no default chain access on ${network}; pass chain.bitcoinEsploraUrl and chain.liquidEsploraUrl`,
      );
    const client = options.makerUrl
      ? new core.SwapClient(options.makerUrl)
      : core.SwapClient.forNetwork(network);
    const base: Omit<Env, "flows"> = {
      network,
      makerUrl,
      chain,
      client,
      store: options.store,
      signer: options.signer,
      pollIntervalMs: options.pollIntervalMs ?? 2_000,
      quoteTtlMs: options.quoteTtlMs ?? 10 * 60_000,
    };
    const env: Env = {
      ...base,
      flows: flows ?? defaultFlows(core, base, options.feeSatPerVb ?? 2),
    };
    return new KaleidoSwapImpl(env);
  };
}
