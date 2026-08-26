// KaleidoSwap swap SDK — TypeScript surface.
//
// Wraps the wasm-bindgen client (bindings-wasm/pkg) with typed signatures. The
// wasm boundary passes plain JS objects (typed `any`); this layer restores the
// domain types with hand-written interfaces.

import initWasm, {
  BoltzClient as WasmBoltzClient,
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

// Boltz swap API client. Re-exported from the wasm module as-is: its request/
// response payloads are currently untyped (`any`) because the Boltz swap DTOs are
// Rust-defined and have no OpenAPI spec to generate TS types from. A typed
// surface would need a schema-generation step (schemars) or hand-written types.
// NOTE: 64-bit integer fields in its responses arrive as `bigint` — the wasm
// boundary serializes Rust i64/u64 losslessly rather than through an f64.
export { BoltzClient } from "../vendor/bindings_wasm.js";

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
}

/**
 * A {@link BoltzClient} that attributes the swaps it creates to a partner
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
 * **Do not call this from code that ships to a browser.** The key is a permanent
 * organization credential with no origin binding and no per-key rate limit, so a
 * key in a browser bundle is visible to every visitor — who can then attribute
 * their own swaps to, or exhaust the limits of, an organization that is not
 * theirs. Nothing in the bundle can prevent that; a publishable attribution key
 * with allowed origins and per-key limits is a separate, later concept. Put the
 * key in server-side configuration, talk to the maker from there, and leave the
 * browser bundle on the unauthenticated `BoltzClient` constructor.
 *
 * One protection is also weaker under `fetch` than on a server: `fetch` owns
 * redirect handling and the SDK can set no policy on it, so a `3xx` away from
 * the maker is reported after the fact instead of declined. The key is not
 * disclosed by such a hop — `fetch` drops `Authorization` when a redirect
 * crosses origins — but the response is not the maker's, and the call fails
 * naming the host that answered.
 */
export function createKaleidoMakerClient(
  options: KaleidoMakerClientOptions,
): WasmBoltzClient {
  return WasmBoltzClient.forKaleidoMaker(options);
}

// WebSocket swap-status stream. Call `runWsLoop()` WITHOUT awaiting (it runs in
// the background), `await subscribeSwap(id)`, then poll `updates().next()`.
// `next()` resolves with a Boltz `SwapStatus` (untyped `any` — Boltz-defined).
export { BoltzWsApi, BoltzWsUpdates } from "../vendor/bindings_wasm.js";

/** Parameters for `SwapScript.constructClaim` / `constructRefund`. */
export interface TxParams {
  /** Where the claimed/refunded funds are sent. */
  outputAddress: string;
  swapId: string;
  /** Per-swap key secret (hex), e.g. `deriveSwapKey(index).secretKey`. */
  keysSecretHex: string;
  boltzBaseUrl: string;
  boltzTimeoutSecs?: number;
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
  boltzBaseUrl: string;
  boltzTimeoutSecs?: number;
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
    return this.inner.constructClaim(preimageHex, params);
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
      params,
      lockupScript.inner,
      refundKeysSecretHex,
    );
  }

  constructRefund(params: TxParams): Promise<BtcLikeTransaction> {
    return this.inner.constructRefund(params);
  }

  async prepareLiquidClaim(
    params: LiquidPsetParams,
  ): Promise<PreparedLiquidSpend> {
    return PreparedLiquidSpend.wrap(
      await this.inner.prepareLiquidClaim(params),
    );
  }

  async prepareLiquidRefund(
    params: LiquidPsetParams,
  ): Promise<PreparedLiquidSpend> {
    return PreparedLiquidSpend.wrap(
      await this.inner.prepareLiquidRefund(params),
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
 * rejected by `BoltzClient.forNetwork` — KaleidoSwap runs no testnet3 maker, and
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
