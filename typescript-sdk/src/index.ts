// KaleidoSwap swap SDK — TypeScript surface.
//
// Wraps the wasm-bindgen client (bindings-wasm/pkg) with typed signatures. The
// wasm boundary passes plain JS objects (typed `any`); this layer restores the
// domain types with hand-written interfaces.

import initWasm, {
  BoltzClient as WasmSwapClient,
  BtcLikeTransaction,
  PreparedLiquidSpend as WasmPreparedLiquidSpend,
  SwapScript as WasmSwapScript,
  WasmSwapMasterKey,
} from "../vendor/bindings_wasm.js";

/** URL of the packaged WebAssembly binary. */
export const wasmUrl = new URL(
  "../vendor/bindings_wasm_bg.wasm",
  import.meta.url,
);

// The swap API client. Re-exported from the wasm module as-is: its request/
// response payloads are currently untyped (`any`) because the swap DTOs are
// Rust-defined and have no OpenAPI spec to generate TS types from. A typed
// surface would need a schema-generation step (schemars) or hand-written types.
// NOTE: 64-bit integer fields in its responses arrive as `bigint` — the wasm
// boundary serializes Rust i64/u64 losslessly rather than through an f64.
//
// It speaks the Boltz protocol and can be pointed at Boltz's own API, which is
// where the old name came from. But the name a partner writes should say whose
// SDK this is, not whose wire format it inherited — that provenance belongs in
// the README and the LICENSE, where it is recorded in full.
export { BoltzClient as SwapClient } from "../vendor/bindings_wasm.js";

export { BtcLikeTransaction };

/** Options for {@link createKaleidoMakerClient}. */
export interface KaleidoMakerClientOptions {
  /** The maker's `/v2` base URL — the only origin the key is ever sent to. */
  makerUrl: string;
  /**
   * The organization API key from the KaleidoSwap partner panel, a
   * `kld_test_…` or `kld_live_…` value.
   */
  apiKey: string;
  /** Per-request timeout in seconds. Omit to leave it to the HTTP stack. */
  timeoutSecs?: number;
  /**
   * Build the client even in a document context. Off by default, and the
   * constructor throws rather than running in a browser — see below. Set this
   * only if you have accepted that every visitor can read the key.
   */
  allowBrowser?: boolean;
}

/**
 * A {@link SwapClient} that attributes the swaps it creates to a partner
 * organization.
 *
 * The key answers *"which partner organization created this swap?"* and nothing
 * else: it authorizes no claim, no refund, no fund movement and no panel access.
 * The per-swap `swapAuth` credential the maker returns on create is what
 * authorizes the outcome of a specific swap, and the two stay separate.
 *
 * The key is bound to `makerUrl` and never sent anywhere else — not to Esplora,
 * not to a second maker. `makerUrl` must be `https` unless it is a loopback
 * address, since a bearer credential over plain HTTP is readable by anything on
 * the path. A value that cannot be a key is rejected here rather than reaching
 * the maker as a `401`, which is the same answer a revoked key gets.
 *
 * ```ts
 * import { init, createKaleidoMakerClient } from "@kaleidorg/swap-sdk";
 *
 * await init();
 * const client = createKaleidoMakerClient({
 *   makerUrl: "https://maker.signet.kaleidoswap.com/v2",
 *   apiKey: process.env.KALEIDOSWAP_API_KEY!,
 * });
 * ```
 *
 * ## Server and native integrations only
 *
 * **This throws if called from a browser.** One wasm artifact serves both Node
 * and the browser, so the check is a runtime one: a document context is
 * refused unless you pass `allowBrowser: true`. The key is a permanent
 * organization credential with no origin binding and no per-key rate limit, so a
 * key in a browser bundle is visible to every visitor — who can then attribute
 * their own swaps to, or exhaust the limits of, an organization that is not
 * theirs. Nothing in the bundle can prevent that; a publishable attribution key
 * with allowed origins and per-key limits is a separate, later concept. Put the
 * key in server-side configuration, talk to the maker from there, and leave the
 * browser bundle on the unauthenticated `SwapClient` constructor.
 *
 * One protection is also weaker under `fetch` than on a server: `fetch` owns
 * redirect handling and the SDK can set no policy on it, so a `3xx` away from
 * the maker is reported after the fact instead of declined. The key is not
 * disclosed by such a hop — `fetch` drops `Authorization` when a redirect
 * crosses origins — but the response is not the maker's, and the call fails
 * naming the host that answered.
 *
 * ## Error reporters that capture locals
 *
 * The key crosses this boundary as a plain `string`, so it is an argument on a
 * stack frame for the length of the call. The SDK keeps it out of its own
 * errors, logs and `toString`, but a reporter configured to capture local
 * variables or function arguments — Sentry's `includeLocalVariables`, and the
 * equivalent in Python (`pytest --showlocals`, `with_locals`) — will capture it
 * from the frame regardless. Scrub `apiKey` in your reporter's
 * before-send hook.
 */
export function createKaleidoMakerClient(
  options: KaleidoMakerClientOptions,
): WasmSwapClient {
  return WasmSwapClient.forKaleidoMaker(options);
}

// WebSocket swap-status stream. Call `runWsLoop()` WITHOUT awaiting (it runs in
// the background), `await subscribeSwap(id)`, then poll `updates().next()`.
// `next()` resolves with a `SwapStatus` (untyped `any` — Boltz-protocol-defined).
export {
  BoltzWsApi as SwapWsApi,
  BoltzWsUpdates as SwapWsUpdates,
} from "../vendor/bindings_wasm.js";

/** Parameters for `SwapScript.constructClaim` / `constructRefund`. */
export interface TxParams {
  /** Where the claimed/refunded funds are sent. */
  outputAddress: string;
  swapId: string;
  /** Per-swap key secret (hex), e.g. `deriveSwapKey(index).secretKey`. */
  keysSecretHex: string;
  makerBaseUrl: string;
  makerTimeoutSecs?: number;
  network: Network;
  bitcoinEsploraUrl?: string;
  liquidEsploraUrl?: string;
  esploraTimeoutSecs?: number;
  /** Relative fee in sat/vByte (mutually exclusive with feeAbsoluteSat). */
  feeSatPerVb?: number;
  /** Absolute fee in satoshis (mutually exclusive with feeSatPerVb). */
  feeAbsoluteSat?: number;
  /**
   * Cooperative (MuSig2 keyspend) claim/refund. Defaults to true.
   *
   * Set `false` for **chain-swap claims** passed to `constructClaim` — that path
   * cannot carry the lockup script the cooperative chain claim signs against.
   * Use `constructCooperativeClaim` instead to get the cheaper keyspend.
   *
   * Refunds need nothing extra: a cooperative refund is co-signed by the server
   * and spends with no locktime, so it does not wait for the timeout.
   */
  cooperative?: boolean;
}

/** Parameters for the caller-funded L-USDT PSET prepare methods. */
export interface LiquidPsetParams {
  outputAddress: string;
  swapId: string;
  /** Application fee ceiling in policy-asset satoshis. */
  maxFee: bigint;
  /** Fee ceiling from the accepted quote. The lower ceiling is pinned. */
  quotedFeeCap: bigint;
  makerBaseUrl: string;
  makerTimeoutSecs?: number;
  network: Network;
  liquidEsploraUrl: string;
  esploraTimeoutSecs?: number;
  /** Optional serialized Liquid lockup transaction for local discovery. */
  lockupTxHex?: string;
}

/** Base64 PSET template and immutable swap intent. */
export interface LiquidPsetTemplate {
  pset: string;
  swapInputIndex: number;
  paymentOutputIndex: number;
  swapAssetId: string;
  policyAssetId: string;
  amount: bigint;
  maxFee: bigint;
}

/** Unblinded data for the designated full-value L-USDT payout. */
export interface LiquidOutputSecrets {
  assetId: string;
  value: bigint;
  assetBlindingFactor: string;
  valueBlindingFactor: string;
}

/** Wallet-funded, blinded and wallet-signed PSET returned for finalization. */
export interface FundedLiquidPset {
  pset: string;
  paymentOutputSecrets: LiquidOutputSecrets;
}

/**
 * Stable error shape for rejections produced after an argument reaches the
 * Rust WASM binding.
 *
 * Input the SDK rejects on the way in — a mistyped argument, an unparseable key,
 * a request object missing a required field — carries the code
 * `"InvalidArgument"` and names the offending argument or field in its message.
 * Failures from the swap engine carry their own code (`"Protocol"`, `"HTTP"`,
 * `"Hex"`, …), while binding-internal failures use `"Internal"`.
 *
 * Values rejected earlier by wasm-bindgen's generated ABI glue remain native
 * JavaScript errors. For example, passing a `number` where a declared `bigint`
 * is required throws `TypeError` before Rust can attach a code.
 */
export interface KaleidoSwapError extends Error {
  readonly code: string;
}

/** Narrow an unknown rejection without parsing its human-readable message. */
export function isKaleidoSwapError(error: unknown): error is KaleidoSwapError {
  return (
    error instanceof Error &&
    typeof (error as { code?: unknown }).code === "string"
  );
}

/** Typed façade over the generated immutable caller-funded Liquid spend. */
export class PreparedLiquidSpend {
  private constructor(private readonly inner: WasmPreparedLiquidSpend) {}

  private static fromWasm(inner: WasmPreparedLiquidSpend): PreparedLiquidSpend {
    return new PreparedLiquidSpend(inner);
  }

  template(): LiquidPsetTemplate {
    return this.inner.template() as LiquidPsetTemplate;
  }

  finalizeClaim(
    fundedPset: FundedLiquidPset,
    keysSecretHex: string,
    preimageHex: string,
  ): BtcLikeTransaction {
    return this.inner.finalizeClaim(fundedPset, keysSecretHex, preimageHex);
  }

  finalizeRefund(
    fundedPset: FundedLiquidPset,
    keysSecretHex: string,
  ): BtcLikeTransaction {
    return this.inner.finalizeRefund(fundedPset, keysSecretHex);
  }

  free(): void {
    this.inner.free();
  }

  static wrap(inner: WasmPreparedLiquidSpend): PreparedLiquidSpend {
    return PreparedLiquidSpend.fromWasm(inner);
  }
}

/** Typed façade over swap reconstruction and transaction construction. */
/**
 * The wasm boundary still deserializes these by their original field names.
 * Renaming them for callers means mapping them back here rather than editing
 * the binding, which keeps the rename in the layer that can be typechecked.
 */
function toWasmParams<T extends TxParams | LiquidPsetParams>(
  params: T,
): Record<string, unknown> {
  const { makerBaseUrl, makerTimeoutSecs, ...rest } = params;
  if (typeof makerBaseUrl !== "string") {
    // Without this the binding rejects the mapped object with "invalid type:
    // unit value, expected a string", which names no field and reads like a
    // bug in the SDK rather than a renamed key in the caller's object.
    const error = new Error(
      "`makerBaseUrl` is required. It was named `boltzBaseUrl` before 0.9.0 — " +
        "rename the field; the old one is ignored.",
    ) as Error & { code: string };
    error.code = "InvalidArgument";
    throw error;
  }
  return {
    ...rest,
    boltzBaseUrl: makerBaseUrl,
    ...(makerTimeoutSecs === undefined
      ? {}
      : { boltzTimeoutSecs: makerTimeoutSecs }),
  };
}

export class SwapScript {
  private constructor(private readonly inner: WasmSwapScript) {}

  static fromSubmarine(
    chainKind: "bitcoin" | "liquid",
    network: Network,
    response: unknown,
    ourPubkeyHex: string,
  ): SwapScript {
    return new SwapScript(
      WasmSwapScript.fromSubmarine(chainKind, network, response, ourPubkeyHex),
    );
  }

  static fromReverse(
    chainKind: "bitcoin" | "liquid",
    network: Network,
    response: unknown,
    ourPubkeyHex: string,
  ): SwapScript {
    return new SwapScript(
      WasmSwapScript.fromReverse(chainKind, network, response, ourPubkeyHex),
    );
  }

  static fromChain(
    chainKind: "bitcoin" | "liquid",
    network: Network,
    side: "lockup" | "claim",
    details: unknown,
    ourPubkeyHex: string,
  ): SwapScript {
    return new SwapScript(
      WasmSwapScript.fromChain(chainKind, network, side, details, ourPubkeyHex),
    );
  }

  constructClaim(
    preimageHex: string,
    params: TxParams,
  ): Promise<BtcLikeTransaction> {
    return this.inner.constructClaim(preimageHex, toWasmParams(params));
  }

  /**
   * Build a **cooperative** chain-swap claim (MuSig2 keyspend).
   *
   * `lockupScript` is our own lockup side —
   * `SwapScript.fromChain(chainKind, network, "lockup", lockupDetails, ourPubkey)`.
   * The cooperative path signs a temporary refund against it to obtain the
   * server's signature for the claim, which is why `constructClaim` cannot do
   * this on its own and needs `cooperative: false` for chain swaps.
   *
   * `refundKeysSecretHex` is the swap's **refund** key — the counterpart of the
   * `refundPublicKey` the swap was created with, not `params.keysSecretHex`. A
   * chain swap carries two independent keys, and the temporary refund is
   * partial-signed with this one. It is a required argument rather than an
   * optional field defaulting to the claim key, because that default is a silent
   * wrong answer for any swap whose two keys differ: the partial signature is
   * made under the wrong key and the server rejects it.
   *
   * The keyspend witness is far smaller than the script path's, and
   * `feeSatPerVb` accounts for that on its own — the fee is computed against a
   * stubbed cooperative witness, so a rate needs no keyspend adjustment.
   *
   * Rejects `params.cooperative === false`; use `constructClaim` for the script
   * path.
   */
  constructCooperativeClaim(
    preimageHex: string,
    params: TxParams,
    lockupScript: SwapScript,
    refundKeysSecretHex: string,
  ): Promise<BtcLikeTransaction> {
    return this.inner.constructCooperativeClaim(
      preimageHex,
      toWasmParams(params),
      lockupScript.inner,
      refundKeysSecretHex,
    );
  }

  constructRefund(params: TxParams): Promise<BtcLikeTransaction> {
    return this.inner.constructRefund(toWasmParams(params));
  }

  async prepareLiquidClaim(
    params: LiquidPsetParams,
  ): Promise<PreparedLiquidSpend> {
    return PreparedLiquidSpend.wrap(
      await this.inner.prepareLiquidClaim(toWasmParams(params)),
    );
  }

  async prepareLiquidRefund(
    params: LiquidPsetParams,
  ): Promise<PreparedLiquidSpend> {
    return PreparedLiquidSpend.wrap(
      await this.inner.prepareLiquidRefund(toWasmParams(params)),
    );
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * `JSON.stringify` that encodes `bigint` values as decimal strings — plain
 * `JSON.stringify` throws on BigInt (the wasm boundary serializes Rust
 * i64/u64 as BigInt so amounts never lose precision through an f64). Use for
 * logging/persisting SDK responses.
 */
// ---------------------------------------------------------------------------
// Arkade Intents corridor — the maker's `/v1` RFQ wire.
//
// `arkade:BTC <-> lightning:BTC` is not a Boltz-shaped route, which is why
// `createReverseSwap({ to: "ARKD" })` is refused at the wasm boundary. The
// maker serves it as an RFQ: post a request, receive a binding quote or a
// refusal, track by `rfq_id`. These types are the corridor's own snake_case
// vocabulary on purpose — a quote here IS the `RfqQuote` that
// `@arkade-os/swap`'s `assertFundable`, `verifyLockupAddress` and
// `deriveLightningReceive` take, so it crosses into `@kaleidorg/swap-sdk/arkade`
// with no rename. 64-bit amounts and timestamps arrive as `bigint`.
//
// This half needs no Arkade dependency: quote, verify, track. Funding a send
// or claiming a receive needs an Ark wallet — that is the venue's job.
// ---------------------------------------------------------------------------

export { corridorRootFromMakerUrl } from "./corridor-url.js";
export { newRfqId } from "../vendor/bindings_wasm.js";

/** The two corridor routes this SDK requests. */
export type ArkadeIntentsPair =
  "arkade:BTC->lightning:BTC" | "lightning:BTC->arkade:BTC";

export const LIGHTNING_SEND_PAIR: ArkadeIntentsPair =
  "arkade:BTC->lightning:BTC";
export const LIGHTNING_RECEIVE_PAIR: ArkadeIntentsPair =
  "lightning:BTC->arkade:BTC";

/** Which leg of the pair an `amount` names. */
export type RfqAmountSide = "from" | "to";

/**
 * `arkade:BTC->lightning:BTC`. Exact-out by construction — the invoice fixes
 * the amount — so there is none to state.
 */
export interface LightningSendRfqRequest {
  /** From {@link newRfqId}. Generate once and carry it. */
  rfq_id: string;
  /** The BOLT11 to be paid; its amount is the swap's `to_amount`. */
  invoice: string;
  /** The trader's own Ark address — where a refund pays. Pinned into a
   * covenant leaf, so an address and not just a key. */
  refund_address: string;
  /** The trader's x-only key (32 bytes, hex) for the sender-side leaves. */
  client_refund_pubkey: string;
}

/**
 * `lightning:BTC->arkade:BTC`. Nothing fixes the size — the maker mints the
 * invoice — so the trader states an amount and which leg it means.
 */
export interface LightningReceiveRfqRequest {
  rfq_id: string;
  /** `"to"` is "receive exactly this on Arkade"; the maker inverts it through
   * its rate card, so `to_amount` rounds *up* by a sat or two — assert `>=`,
   * never equality. */
  amount_side: RfqAmountSide;
  /** Sats, on the `amount_side` leg. */
  amount: bigint | number;
  /** `sha256(P)` of the trader's OWN preimage, hex. */
  payment_hash: string;
  /** The trader's Arkade payout address — pins the claim leaf. */
  payout_address: string;
  /** The trader's x-only Arkade key: the covenant's `receiver`. */
  payout_pubkey: string;
  /** Pre-signed claim for the maker's claim daemon; omit where none runs. */
  claim_packet?: string;
}

/** The route-specific half of a quote. A send fills `lockup_address` and
 * `receiver_pk_script`; a receive fills `lockup_address`, `invoice` and
 * `solver_refund_pk_script`. */
export interface RfqQuoteProfile {
  payment_hash?: string;
  /** Compare-only: derive the same covenant from the binding fields and
   * refuse on mismatch. */
  lockup_address?: string;
  /** Receive: the hold invoice the trader pays to arm the swap. */
  invoice?: string;
  /** Send: the maker's payout destination, pinned into the claim leaf. */
  receiver_pk_script?: string;
  /** Receive: the maker's refund destination, pinned into the refund leaf. */
  solver_refund_pk_script?: string;
}

/** The binding answer. Funding it is the acceptance — every field is final. */
export interface RfqQuote {
  v: number;
  type: "rfq_quote";
  rfq_id: string;
  pair: string;
  /** What the trader gives, sats. */
  from_amount: bigint;
  /** What the trader receives, sats. The fee is the spread. */
  to_amount: bigint;
  solver_pubkey: string;
  /** Unix seconds after which a fresh quote is needed. */
  valid_until: bigint;
  /** Absolute refund deadline, unix seconds — the trader's on a send, the
   * maker's on a receive. */
  refund_locktime?: bigint;
  profile: RfqQuoteProfile;
}

/** The maker's vocabulary; `"unknown"` is a reason this SDK version has not
 * heard of, still a refusal. */
export type RfqRefusalReason =
  | "unsupported_pair"
  | "unsupported_payload"
  | "amount_out_of_range"
  | "exposure_cap"
  | "invoice_expired"
  | "quote_conflict"
  | "pricing_unavailable"
  | "unknown";

export interface RfqRefusal {
  v: number;
  type: "rfq_refusal";
  rfq_id: string;
  reason: RfqRefusalReason;
}

/** What `POST /v1/swap` answers. A refusal is a `200` and the maker's
 * decision — read `type`, do not treat it as a thrown error. */
export type RfqAnswer = RfqQuote | RfqRefusal;

export function isRfqQuote(answer: RfqAnswer): answer is RfqQuote {
  return answer.type === "rfq_quote";
}

export type RfqState =
  | "quoted"
  | "refused"
  | "expired"
  | "funded"
  | "filling"
  | "filled"
  | "settled"
  | "refunded"
  | "stuck";

/** The states after which no further update comes — poll until one. */
export const RFQ_TERMINAL_STATES: ReadonlySet<RfqState> = new Set<RfqState>([
  "settled",
  "refused",
  "expired",
  "refunded",
  "stuck",
]);

export interface RfqStatus {
  v: number;
  type: "rfq_status";
  rfq_id: string;
  state: RfqState;
  /** Unix seconds. */
  updated_at: bigint;
  profile: RfqQuoteProfile;
}

/**
 * Typed façade over a {@link SwapClient}'s corridor methods.
 *
 * ```ts
 * const corridor = new IntentsCorridor(SwapClient.forNetwork("signet"));
 * const answer = await corridor.quoteLightningSend({
 *   rfq_id: newRfqId(), invoice, refund_address, client_refund_pubkey,
 * });
 * if (!isRfqQuote(answer)) throw new Error(`refused: ${answer.reason}`);
 * // Derive the covenant from `answer` with @kaleidorg/swap-sdk/arkade, compare
 * // against answer.profile.lockup_address, then fund answer.from_amount.
 * ```
 */
export class IntentsCorridor {
  constructor(private readonly client: WasmSwapClient) {}

  /** The origin the corridor hangs off — the client's `/v2` base minus the
   * suffix. Same rule as {@link corridorRootFromMakerUrl}. */
  get url(): string {
    return this.client.corridorUrl as string;
  }

  quoteLightningSend(request: LightningSendRfqRequest): Promise<RfqAnswer> {
    return this.client.quoteLightningSend(request) as Promise<RfqAnswer>;
  }

  quoteLightningReceive(
    request: LightningReceiveRfqRequest,
  ): Promise<RfqAnswer> {
    return this.client.quoteLightningReceive(request) as Promise<RfqAnswer>;
  }

  /** `null` for an id the maker never issued. */
  status(rfqId: string): Promise<RfqStatus | null> {
    return this.client.rfqStatus(rfqId) as Promise<RfqStatus | null>;
  }
}

export function toJson(value: unknown, space?: string | number): string {
  return JSON.stringify(
    value,
    (_key, v: unknown) => (typeof v === "bigint" ? v.toString() : v),
    space,
  );
}

/**
 * Sources accepted for the compiled WebAssembly binary.
 *
 * Deliberately narrower than wasm-bindgen's own `InitInput`, which also admits
 * `WebAssembly.Module`. TypeScript declares that as an *empty* interface, and an
 * empty interface is structurally assignable from any non-nullish value — so a
 * union containing it silently accepts `42` or `"nonsense"` and voids
 * type-checking for every other member. Callers holding a pre-compiled module
 * can still pass it through {@link initWithModule}.
 */
export type WasmSource = BufferSource | URL | Request | Response | string;

// Compile-time guard for the note above. Emits nothing. If `WasmSource` is ever
// widened back to a union containing an empty interface (`WebAssembly.Module`,
// or `{}`), a primitive becomes assignable to it and this fails to compile —
// which is the only signal, since such a union still *looks* precise.
type Assert<T extends true> = T;
// eslint-disable-next-line @typescript-eslint/no-unused-vars -- the assertion IS the test
type _WasmSourceRejectsPrimitives = Assert<
  42 extends WasmSource ? false : true
>;

/**
 * Load and instantiate the wasm module. Call once (await it) before creating any
 * client.
 *
 * Takes no argument in normal use: browsers resolve the packaged binary relative
 * to this module, and the Node entry point (selected automatically via the
 * `"node"` export condition) reads it from disk. Pass a {@link WasmSource} only
 * to override that — for example to serve the binary from your own CDN.
 */
export async function init(
  source?: WasmSource | Promise<WasmSource>,
): Promise<void> {
  await initWasm(source === undefined ? undefined : { module_or_path: source });
}

/**
 * Initialize from a pre-compiled `WebAssembly.Module`. Separate from
 * {@link init} so that {@link WasmSource} can stay type-safe — see the note on
 * that type.
 */
export async function initWithModule(
  module: WebAssembly.Module,
): Promise<void> {
  await initWasm({ module_or_path: module });
}

/** A derived swap keypair (hex). */
export interface DerivedKey {
  publicKey: string;
  secretKey: string;
}

/** A derived swap preimage and its hashes (hex). */
export interface DerivedPreimage {
  preimage: string;
  sha256: string;
  hash160: string;
}

/**
 * `"signet"` is the KaleidoSwap maker's network. It settles on Mutinynet, so
 * pair it with Mutinynet chain access
 * (`https://esplora.signet.kaleidoswap.com`, our own Esplora), never a
 * testnet3 endpoint: signet and testnet3 encode addresses identically, so the
 * mismatch raises no error — swaps are simply created on one chain and funded
 * or watched on another. `"testnet"` is testnet3, usable as a chain identity but
 * rejected by `SwapClient.forNetwork` — KaleidoSwap runs no testnet3 maker, and
 * defaults never fall back to a third-party one.
 */
export type Network = "mainnet" | "testnet" | "signet" | "regtest";

/** Client-side swap key derivation (BIP85 index 26589 over a wallet mnemonic). */
export class SwapMasterKey {
  private constructor(private readonly inner: WasmSwapMasterKey) {}

  static fromWalletMnemonic(
    walletMnemonic: string,
    network: Network,
    passphrase?: string,
  ): SwapMasterKey {
    return new SwapMasterKey(
      WasmSwapMasterKey.fromWalletMnemonic(walletMnemonic, passphrase, network),
    );
  }

  static fromSwapMnemonic(
    mnemonic: string,
    network: Network,
    passphrase?: string,
  ): SwapMasterKey {
    return new SwapMasterKey(
      WasmSwapMasterKey.fromSwapMnemonic(mnemonic, passphrase, network),
    );
  }

  swapMnemonic(): string {
    return this.inner.swapMnemonic();
  }
  masterXpub(): string {
    return this.inner.masterXpub();
  }
  deriveSwapKey(index: bigint): DerivedKey {
    return this.inner.deriveSwapKey(index);
  }
  derivePreimage(index: bigint): DerivedPreimage {
    return this.inner.derivePreimage(index);
  }
}
