// KaleidoSwap — the high-level surface. The phase machine, the pricing and
// every check live in the Rust core (`kaleidoswap::kaleidoswap`) and
// reach this package through the wasm binding; this file declares the types
// callers write against and adapts the store and the signer they provide.

import type { DerivedPreimage, Network } from "./index.js";
import type { MakerAttribution } from "./partner-key.js";

// ---------------------------------------------------------------------------
// Assets and amounts

/** The layer an asset lives on. */
export type Layer = "bitcoin" | "lightning" | "liquid" | "arkade" | "spark";

/**
 * What a swap pays or receives, and where it lives: `<layer>:<asset>`.
 * Spelling the layer is what makes a wrong-network destination a local error
 * instead of an unrecoverable payout. RGB contracts and Spark tokens are
 * named by id (`bitcoin:rgb:…`, `spark:btkn…`).
 */
export type Asset =
  | "bitcoin:BTC"
  | "lightning:BTC"
  | "liquid:BTC"
  | "liquid:USDT"
  | "arkade:BTC"
  | "spark:BTC"
  | `bitcoin:rgb:${string}`
  | `lightning:rgb:${string}`
  | `spark:${string}`;

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
 * A cooperative MuSig2 key-path signing session, hex-encoded: the two
 * compressed public keys in aggregation order, the Taproot tweak and the
 * sighash.
 */
export interface MusigSession {
  pubkeys: [string, string];
  tweak: string;
  sighash: string;
}

type MaybePromise<T> = T | Promise<T>;

/**
 * Holds the per-swap keys, one per store index, and signs with them. The SDK
 * asks for public keys, preimages and signatures, never a secret key, so a
 * wallet can keep its keys in a hardware device, an enclave or a remote
 * signer. `mnemonicSigner` is the in-memory implementation. Hex in, hex out.
 *
 * `musigNonce` generates and keeps a fresh secret nonce and returns the public
 * nonce; `musigPartialSign` signs with the secret nonce behind `ourNonce` and
 * forgets it. Never sign twice with one nonce, and never take a secret nonce
 * from outside: either reveals the key.
 */
export interface SwapSigner {
  /** The compressed public key of swap `index`. */
  publicKey(index: bigint | number): MaybePromise<string>;
  /** The preimage of swap `index`. */
  derivePreimage(
    index: bigint | number,
  ): MaybePromise<Pick<DerivedPreimage, "preimage">>;
  /** A BIP340 signature by the untweaked key over a script-path sighash. */
  signSchnorr(index: bigint | number, sighash: string): MaybePromise<string>;
  /** Starts a MuSig2 signature for `session`: the public nonce. */
  musigNonce(
    index: bigint | number,
    session: MusigSession,
  ): MaybePromise<string>;
  /** Finishes it: the partial signature. */
  musigPartialSign(
    index: bigint | number,
    session: MusigSession,
    ourNonce: string,
    theirNonce: string,
  ): MaybePromise<string>;
}

// ---------------------------------------------------------------------------
// Construction

export interface KaleidoSwapOptions {
  network: Network;
  /**
   * Where swaps are kept until they end. A {@link SwapStore}, or on the Node
   * entry a directory path, which becomes a `FileStore` over it.
   */
  store: SwapStore | string;
  /** Who signs. Give this or `mnemonic`, not both. */
  signer?: SwapSigner;
  /**
   * The swap mnemonic, for a caller who holds it in memory: shorthand for
   * `signer: mnemonicSigner(mnemonic, network)`.
   */
  mnemonic?: string;
  /** The maker's `/v2` base URL. Defaults per network exactly as `SwapClient.forNetwork` does. */
  makerUrl?: string;
  /**
   * The partner's `kld_` key, a public partner identifier: ship it in the app
   * as a build constant. It is sent to the maker, and only to the maker, on
   * every pair-card read and every create, so quotes are priced from the
   * partner's own card (the partner fee shows as `fees.partner`) and the
   * swaps are attributed to the partner. Omit it for the public price and no
   * attribution.
   */
  apiKey?: string;
  /** Defaults to the KaleidoSwap endpoints for `network`; required where there are none. */
  bitcoinEsploraUrl?: string;
  liquidEsploraUrl?: string;
  /** How often a swap in flight is asked for its status. Default 2 s. */
  pollIntervalMs?: number;
  /** Timeout of every request to the maker. Default 30 s. */
  makerTimeoutMs?: number;
  /**
   * A fixed fee rate for the claims and refunds this layer signs, on either
   * chain. When set, no estimate is read; the rate is raised to the floor of
   * the chain a spend is on, and a rate below both floors is refused.
   * Default: estimated, see `feePolicy`.
   */
  feeSatPerVb?: number;
  /** How claims and refunds are priced when `feeSatPerVb` is not set. */
  feePolicy?: FeePolicy;
  /**
   * How long a quote may be held before `start` refuses it. Pair terms are
   * bound by hash, so this guards staleness, not price. Default 10 minutes.
   */
  quoteTtlMs?: number;
  /**
   * Blocks a reverse swap's lockup must have left before its timeout for
   * `start` to hand out the invoice: room to confirm, then claim. Defaults
   * to 12 on Bitcoin and 120 on Liquid, twice the claim path's safety margin;
   * a smaller value is raised to one and a half times that margin (9 and 90).
   */
  reverseTimeoutHeadroom?: { bitcoinBlocks?: number; liquidBlocks?: number };
}

/**
 * The rate is read from the chain client's `/fee-estimates` for
 * `targetBlocks` when a claim or refund is about to be signed, and held
 * between the chain's floor and ceiling. An estimate that cannot be read signs
 * nothing: the spend fails with `CHAIN_UNREACHABLE` and is retried on the next
 * poll. A spend whose fee would exceed `maxFeePercent` of the lockup is not
 * signed either: it fails with `FEE_TOO_HIGH`, before a claim hands out the
 * preimage, and is retried. A claim already under way is always signed, at no
 * less than the rate its first attempt was signed at, held under 50% of the
 * lockup (or `maxFeePercent` if higher). Every field left out keeps its default.
 */
export interface FeePolicy {
  /** Confirmation target in blocks; the nearest estimate at or below it is used. Default 2. */
  targetBlocks?: number;
  /** Default 1 sat/vB. */
  bitcoinMinSatPerVb?: number;
  /** Default 500 sat/vB. */
  bitcoinMaxSatPerVb?: number;
  /** Default 0.1 sat/vB. */
  liquidMinSatPerVb?: number;
  /** Default 1 sat/vB. */
  liquidMaxSatPerVb?: number;
  /** The largest share of the lockup a claim or refund may pay in fees, in percent. Default 10. */
  maxFeePercent?: number;
}

// ---------------------------------------------------------------------------
// Quote

/**
 * What fixes a quote's size: what you pay, what you receive, or, on a swap
 * to Lightning, the invoice the maker pays (its amount is the `receive` side).
 */
export type QuoteAmount =
  { pay: Amount } | { receive: Amount } | { invoice: string };

/**
 * Both sides are the caller's to name; the protocol follows from them and is
 * reported on the {@link Quote}, never inferred from an address or invoice.
 */
export interface QuoteRequest {
  /** What you pay, and where it lives. */
  pay: Asset;
  /** What you receive, and where it lives. */
  receive: Asset;
  amount: QuoteAmount;
}

/**
 * The same request in the words a {@link Quote} answers with: `from` and `to`,
 * and exactly one of `fromAmount` (what you pay), `toAmount` (what lands) or
 * `invoice` (the Lightning invoice the maker pays).
 */
export interface SwapTerms {
  from: Asset;
  to: Asset;
  fromAmount?: Amount;
  toAmount?: Amount;
  invoice?: string;
}

/** {@link SwapTerms} plus where the swap lands and where a refund goes: one call's worth. */
export interface SwapRequest extends SwapTerms, StartOptions {}

/** The wire a quote rides on. Informational; the caller never picks one. */
export type Protocol =
  "boltz-submarine" | "boltz-reverse" | "arkade-rfq" | "pay-through";

export interface Quote {
  kind: "quote";
  protocol: Protocol;
  from: Asset;
  to: Asset;
  fromAmount: bigint;
  toAmount: bigint;
  /**
   * `fromAmount − toAmount = maker + partner + miner`. `partner` is the fee
   * the partner whose `apiKey` this instance carries charges on top of the
   * maker's price: `0n` without a key.
   */
  fees: { maker: bigint; miner: bigint; partner: bigint };
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
  from: Asset;
  to: Asset;
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
  from: Asset;
  to: Asset;
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
  /**
   * The maker's BIP21 URI for the same funding, set only when it names
   * exactly `fundingAddress` and `fundingAmount` and nothing a wallet could
   * pay instead.
   */
  bip21?: string;
  /**
   * The largest lockup the maker accepts unconfirmed. Zero means the maker
   * waits for a confirmation before acting; absent, that it is not known.
   */
  zeroConfLimit?: bigint;
}

/**
 * Where a swap stands, for a caller who shows it to a user. Each
 * {@link SwapPhase} maps to one; see {@link statusOf}.
 */
export type SwapStatus =
  | "waiting_for_payment"
  | "processing"
  | "completed"
  | "refunding"
  | "refunded"
  | "failed";

export function statusOf(phase: SwapPhase): SwapStatus {
  switch (phase) {
    case "created":
    case "awaiting_funding":
      return "waiting_for_payment";
    case "funded":
    case "locked":
    case "claiming":
      return "processing";
    case "settled":
      return "completed";
    case "refundable":
    case "refunding":
      return "refunding";
    case "refunded":
      return "refunded";
    case "failed":
      return "failed";
  }
}

/** What you send to fund the swap: {@link SwapAction} told apart by `kind`. */
export type SwapDeposit =
  | { kind: "lightning"; invoice: string }
  | { kind: "onchain"; address: string; amount: bigint; uri?: string };

export interface Swap {
  readonly id: string;
  readonly protocol: Protocol;
  readonly phase: SwapPhase;
  readonly action: SwapAction;
  /** {@link phase} as one of six states. */
  readonly status: SwapStatus;
  /** What to pay or fund, or `undefined` once there is nothing left to send. */
  readonly deposit: SwapDeposit | undefined;

  /** {@link phases} as statuses: each status once, ending on a terminal one. */
  updates(signal?: AbortSignal): AsyncIterable<SwapStatus>;
  /** {@link settled} under the name {@link SwapStatus} uses. */
  completed(signal?: AbortSignal): Promise<SwapRecord>;

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
  | "ABORTED"
  | "CHAIN_UNREACHABLE"
  | "INVOICE_EXPIRED"
  | "FEE_TOO_HIGH";

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
  "CHAIN_UNREACHABLE",
  "INVOICE_EXPIRED",
  "FEE_TOO_HIGH",
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
  /**
   * Swaps this pass could not move — the maker did not know them, a claim was
   * refused — each with its error. They stay in the store, and one of them
   * never stops the others from being resumed.
   */
  errors: { id: string; code: SwapErrorCode; message: string }[];
}

export interface KaleidoSwap {
  readonly network: Network;

  /**
   * What the maker last answered in `X-Kaleido-Attribution`: `"api_key"`
   * when it priced or stored the request for `apiKey`'s partner, `"none"`
   * when it did not recognise the key (unknown, revoked or for the other
   * environment) and answered with the public price. `undefined` until a
   * response carried it.
   */
  readonly lastAttribution: MakerAttribution | undefined;

  quote(request: QuoteRequest | SwapTerms): Promise<QuoteAnswer>;

  /**
   * Creates the swap against `quote`, verifies the lockup against our own
   * key and, for a reverse swap, that its timeout leaves room to confirm and
   * claim, persists the record, then returns. Nothing is funded or paid here
   * — that is the caller's step, via {@link Swap.action}.
   */
  start(quote: Quote, options?: StartOptions): Promise<Swap>;

  /** A swap this instance or an earlier one started, by id. */
  swap(id: string): Promise<Swap>;
  /**
   * `quote` then `start` in one call, for a caller who does not need to show
   * the price first. A refusal rejects with a {@link SwapError}:
   * `PAIR_UNAVAILABLE`, `AMOUNT_OUT_OF_LIMITS` or `MAKER_REJECTED`.
   */
  swap(request: SwapRequest): Promise<Swap>;

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
  readonly lastAttribution: string | undefined;
  quote(request: unknown): Promise<unknown>;
  start(quote: unknown, options?: unknown): Promise<WasmSwap>;
  swap(id: string): Promise<WasmSwap>;
  resume(): Promise<{
    resumed: WasmSwap[];
    settled: SwapRecord[];
    refunded: SwapRecord[];
    failed: SwapRecord[];
    errors: ResumeReport["errors"];
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
  /** Builds the signer for the `mnemonic` option. */
  mnemonicSigner(mnemonic: string, network: Network): SwapSigner;
  /** Builds the store for a `store` given as a directory; absent where there is no filesystem. */
  fileStore?(dir: string): SwapStore;
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

function depositOf(action: SwapAction): SwapDeposit | undefined {
  if (action.invoice !== undefined)
    return { kind: "lightning", invoice: action.invoice };
  if (action.fundingAddress !== undefined && action.fundingAmount !== undefined)
    return {
      kind: "onchain",
      address: action.fundingAddress,
      amount: action.fundingAmount,
      ...(action.bip21 !== undefined && { uri: action.bip21 }),
    };
  return undefined;
}

/** A request in either spelling, as the `{ pay, receive, amount }` the core takes. */
function quoteRequestOf(request: QuoteRequest | SwapTerms): QuoteRequest {
  if (!("from" in request) && !("to" in request)) return request;
  const { from, to, fromAmount, toAmount, invoice } = request as SwapTerms;
  const given = [
    fromAmount !== undefined && { pay: fromAmount },
    toAmount !== undefined && { receive: toAmount },
    invoice !== undefined && { invoice },
  ].filter((amount): amount is QuoteAmount => amount !== false);
  if (given.length !== 1)
    throw swapError(
      "INVALID_AMOUNT",
      "give exactly one of fromAmount, toAmount or invoice",
    );
  return { pay: from, receive: to, amount: given[0] };
}

const REFUSAL_CODES: Record<Refusal["reason"], SwapErrorCode> = {
  pair_unavailable: "PAIR_UNAVAILABLE",
  amount_below_min: "AMOUNT_OUT_OF_LIMITS",
  amount_above_max: "AMOUNT_OUT_OF_LIMITS",
  no_liquidity: "MAKER_REJECTED",
  maker_paused: "MAKER_REJECTED",
};

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
    get status() {
      return statusOf(inner.phase);
    },
    get deposit() {
      return depositOf(inner.action);
    },
    record: () => inner.record(),
    refund: () => inner.refund(),
    settled: (signal) => raceAbort(inner.settled(), signal),
    completed: (signal) => raceAbort(inner.settled(), signal),
    async *updates(signal) {
      let last: SwapStatus | undefined;
      for await (const phase of this.phases(signal)) {
        const status = statusOf(phase);
        if (status !== last) yield (last = status);
      }
    },
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
    // The inner promise gets its handlers first, so that when the signal has
    // already fired its later rejection is still handled.
    promise
      .then(resolve, reject)
      .finally(() => signal.removeEventListener("abort", abort));
    if (signal.aborted) return abort();
    signal.addEventListener("abort", abort, { once: true });
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
    const { mnemonic, ...rest } = options;
    if ((mnemonic === undefined) === (options.signer === undefined))
      throw new TypeError("give exactly one of signer or mnemonic");
    let store = options.store;
    if (typeof store === "string") {
      if (!core.fileStore)
        throw new TypeError(
          "store as a directory path needs the Node entry; pass a SwapStore",
        );
      store = core.fileStore(store);
    }
    const inner = new core.KaleidoSwap({
      ...rest,
      store,
      signer: options.signer ?? core.mnemonicSigner(mnemonic!, options.network),
    });
    const quote = async (
      request: QuoteRequest | SwapTerms,
    ): Promise<QuoteAnswer> => {
      const { pay, receive, amount: given } = quoteRequestOf(request);
      const amount = given as QuoteAmount | undefined;
      if (typeof amount !== "object" || amount === null)
        throw swapError(
          "INVALID_AMOUNT",
          "amount is required: { pay }, { receive } or { invoice }",
        );
      const wire = {
        pay,
        receive,
        amount:
          "pay" in amount
            ? { pay: amountToBigint(amount.pay, "amount.pay") }
            : "receive" in amount
              ? { receive: amountToBigint(amount.receive, "amount.receive") }
              : amount,
      };
      return (await inner.quote(wire)) as QuoteAnswer;
    };
    const start = async (answer: Quote, startOptions?: StartOptions) =>
      wrapSwap(await inner.start(answer, startOptions));
    return {
      get network() {
        return options.network;
      },
      get lastAttribution() {
        return inner.lastAttribution;
      },
      quote,
      start,
      async swap(request: string | SwapRequest) {
        if (typeof request === "string")
          return wrapSwap(await inner.swap(request));
        const { destination, refundAddress, ...terms } = request;
        const answer = await quote(terms);
        if (!isQuote(answer))
          throw swapError(
            REFUSAL_CODES[answer.reason] ?? "MAKER_REJECTED",
            `${answer.reason}: ${answer.message}`,
          );
        return start(answer, { destination, refundAddress });
      },
      async resume() {
        const report = await inner.resume();
        return { ...report, resumed: report.resumed.map(wrapSwap) };
      },
    };
  };
}
