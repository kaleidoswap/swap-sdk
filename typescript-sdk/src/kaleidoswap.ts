// KaleidoSwap — the high-level surface. The phase machine, the pricing and
// every check live in the Rust core (`kaleidorg_swap_sdk::kaleidoswap`) and
// reach this package through the wasm binding; this file declares the types
// callers write against and adapts the store and the signer they provide.

import type { DerivedKey, DerivedPreimage, Network } from "./index.js";

// ---------------------------------------------------------------------------
// Assets and amounts

/** The layer an asset lives on. `arkade` is Bitcoin on Arkade (`ARKD`). */
export type Layer = "bitcoin" | "lightning" | "liquid" | "arkade";

/**
 * `<layer>:<asset>`. Spelling the layer is what makes a wrong-network
 * destination a local error instead of an unrecoverable payout.
 */
export type AssetId =
  "bitcoin:BTC" | "lightning:BTC" | "liquid:BTC" | "liquid:USDT" | "arkade:BTC";

/**
 * Atomic units of the asset — sats for BTC on any layer, L-USDT units for
 * `liquid:USDT`. A `number` must be a safe integer; a `string` is parsed as
 * one. Everything is `bigint` past this boundary.
 */
export type Amount = bigint | number | string;

// ---------------------------------------------------------------------------
// Injected dependencies

/**
 * Where the SDK keeps everything a swap needs to finish or unwind after a
 * restart. Records are plain JSON-compatible objects — numbers, no `bigint` —
 * so persisting them verbatim is the whole job. Every write lands before the
 * irreversible step it precedes. Methods may return a value or a promise.
 */
export interface SwapStore {
  get(id: string): SwapRecord | undefined | Promise<SwapRecord | undefined>;
  put(record: SwapRecord): void | Promise<void>;
  list(): SwapRecord[] | Promise<SwapRecord[]>;
  /**
   * Hands out the next BIP85 child index, exactly once. An index reused is
   * claim and refund material reused, so this has to be atomic for the store.
   */
  nextIndex(): bigint | number | Promise<bigint | number>;
}

/**
 * Derives per-swap material from a root the caller owns. `mnemonicSigner`
 * wraps `SwapMasterKey`; a wallet that keeps its seed elsewhere implements
 * this instead. The SDK never sees a mnemonic through it.
 */
export interface SwapSigner {
  deriveKey(index: bigint | number): DerivedKey | Promise<DerivedKey>;
  derivePreimage(
    index: bigint | number,
  ):
    | Pick<DerivedPreimage, "preimage">
    | Promise<Pick<DerivedPreimage, "preimage">>;
}

// ---------------------------------------------------------------------------
// Construction

export interface KaleidoSwapOptions {
  network: Network;
  store: SwapStore;
  signer: SwapSigner;
  /** The maker's `/v2` base URL. Defaults per network exactly as `SwapClient.forNetwork` does. */
  makerUrl?: string;
  /** Defaults to the KaleidoSwap endpoints for `network`; required where there are none. */
  bitcoinEsploraUrl?: string;
  liquidEsploraUrl?: string;
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
  /** Unix seconds. Accepting after this is a new quote, not this one. */
  expiresAt: bigint;
  /** What holds the maker to these terms. Pass the quote back to `start` untouched. */
  binding: unknown;
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
}

// ---------------------------------------------------------------------------
// Swap lifecycle

/**
 * One state machine over every protocol. The engine's per-protocol statuses
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
  createdAt: number;
  updatedAt: number;
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
  refund(): Promise<SwapRecord>;
  /** The record as this handle last saw it. */
  record(): SwapRecord;
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

/** Every failure crosses this boundary coded; the engine's errors are mapped in Rust. */
export interface SwapError extends Error {
  code: SwapErrorCode;
  swapId?: string;
}

const SWAP_ERROR_CODES: ReadonlySet<string> = new Set<SwapErrorCode>([
  "INVALID_AMOUNT",
  "AMOUNT_OUT_OF_LIMITS",
  "PAIR_UNAVAILABLE",
  "QUOTE_EXPIRED",
  "WRONG_NETWORK_ADDRESS",
  "INVALID_DESTINATION",
  "REFUND_ADDRESS_REQUIRED",
  "LOCKUP_MISMATCH",
  "MAKER_UNREACHABLE",
  "MAKER_REJECTED",
  "SWAP_EXPIRED",
  "SWAP_FAILED",
  "SWAP_NOT_FOUND",
  "STORE_FAILED",
  "ABORTED",
]);

export function isSwapError(error: unknown): error is SwapError {
  return (
    error instanceof Error &&
    typeof (error as Partial<SwapError>).code === "string" &&
    SWAP_ERROR_CODES.has((error as SwapError).code)
  );
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
  resume(): Promise<ResumeReport>;
}

/** In-memory store — tests and throwaway scripts. Nothing survives the process. */
export class MemoryStore implements SwapStore {
  private readonly records = new Map<string, SwapRecord>();
  private index = 0n;

  constructor(startIndex: bigint = 0n) {
    this.index = startIndex;
  }

  get(id: string): SwapRecord | undefined {
    return this.records.get(id);
  }
  put(record: SwapRecord): void {
    this.records.set(record.id, record);
  }
  list(): SwapRecord[] {
    return [...this.records.values()];
  }
  nextIndex(): bigint {
    return this.index++;
  }
}

/** A coded error of the kind the Rust core raises, for the checks that stay on the JS side. */
export function swapError(code: SwapErrorCode, message: string): SwapError {
  const error = new Error(message) as SwapError;
  error.code = code;
  error.name = code;
  return error;
}

// ---------------------------------------------------------------------------
// Adapters over the wasm binding

/** The wasm classes this layer wraps; handed in so each entry binds its own `init`. */
export interface WasmKaleidoSwap {
  readonly network: string;
  quote(request: unknown): Promise<unknown>;
  start(quote: unknown, options?: unknown): Promise<WasmSwap>;
  swap(id: string): Promise<WasmSwap>;
  resume(): Promise<{
    resumed: WasmSwap[];
    settled: SwapRecord[];
    refunded: SwapRecord[];
    failed: SwapRecord[];
  }>;
}

export interface WasmSwap {
  readonly id: string;
  readonly protocol: Protocol;
  readonly phase: SwapPhase;
  readonly action: SwapAction;
  record(): SwapRecord;
  tick(): Promise<SwapRecord>;
  refund(): Promise<SwapRecord>;
  watch(onPhase?: (phase: SwapPhase) => void): Promise<SwapRecord>;
  settled(): Promise<SwapRecord>;
}

export interface WasmKaleidoSwapCore {
  init(): Promise<void>;
  KaleidoSwap: new (options: unknown) => WasmKaleidoSwap;
}

/** An amount in any accepted form, as the `bigint` the binding takes. */
function amountToBigint(value: Amount, what: string): bigint {
  const invalid = (detail: string) => {
    const error = new Error(`${what} ${detail}`) as SwapError;
    error.code = "INVALID_AMOUNT";
    error.name = "INVALID_AMOUNT";
    return error;
  };
  if (typeof value === "bigint") return value;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0)
      throw invalid(
        `must be a nonnegative safe integer in atomic units, got ${value}`,
      );
    return BigInt(value);
  }
  if (/^\d+$/.test(value)) return BigInt(value);
  throw invalid(`is not an integer: ${value}`);
}

function wrapSwap(inner: WasmSwap): Swap {
  return {
    get id() {
      return inner.id;
    },
    get protocol() {
      return inner.protocol;
    },
    get phase() {
      return inner.phase;
    },
    get action() {
      return inner.action;
    },
    record: () => inner.record(),
    refund: () => inner.refund(),
    settled: (signal) => raceAbort(inner.settled(), signal),
    async *phases(signal) {
      const queue: SwapPhase[] = [];
      let wake: (() => void) | undefined;
      let finished = false;
      let failure: unknown;
      const run = inner
        .watch((phase) => {
          queue.push(phase);
          wake?.();
        })
        .then(
          () => undefined,
          (error: unknown) => {
            failure = error;
          },
        )
        .finally(() => {
          finished = true;
          wake?.();
        });
      try {
        for (;;) {
          if (signal?.aborted) {
            const error = new Error("the operation was aborted") as SwapError;
            error.code = "ABORTED";
            throw error;
          }
          while (queue.length > 0) yield queue.shift()!;
          if (finished) break;
          await new Promise<void>((resolve) => {
            wake = resolve;
            signal?.addEventListener("abort", () => resolve(), { once: true });
          });
          wake = undefined;
        }
        if (failure !== undefined && !TERMINAL_PHASES.has(inner.phase))
          throw failure;
      } finally {
        void run;
      }
    },
  };
}

/** Rejects with `ABORTED` when `signal` fires; the swap itself keeps going and is resumable. */
function raceAbort<T>(promise: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return promise;
  return new Promise<T>((resolve, reject) => {
    const abort = () => {
      const error = new Error("the operation was aborted") as SwapError;
      error.code = "ABORTED";
      error.name = "ABORTED";
      reject(error);
    };
    if (signal.aborted) return abort();
    signal.addEventListener("abort", abort, { once: true });
    promise
      .then(resolve, reject)
      .finally(() => signal.removeEventListener("abort", abort));
  });
}

/**
 * Builds the constructor over the wasm core. Each entry point exports the
 * result, so `init()` is the one that entry's loader provides.
 */
export function makeCreateKaleidoSwap(
  core: WasmKaleidoSwapCore,
): (options: KaleidoSwapOptions) => Promise<KaleidoSwap> {
  return async (options) => {
    await core.init();
    const inner = new core.KaleidoSwap(options);
    return {
      get network() {
        return options.network;
      },
      async quote(request) {
        const wire = {
          ...request,
          amount:
            request.amount === undefined
              ? undefined
              : amountToBigint(request.amount, "amount"),
        };
        return (await inner.quote(wire)) as QuoteAnswer;
      },
      async start(quote, startOptions) {
        return wrapSwap(await inner.start(quote, startOptions));
      },
      async swap(id) {
        return wrapSwap(await inner.swap(id));
      },
      async resume() {
        const report = await inner.resume();
        return { ...report, resumed: report.resumed.map(wrapSwap) };
      },
    };
  };
}
