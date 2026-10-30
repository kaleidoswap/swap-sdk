// KaleidoSwap swap SDK — TypeScript surface.
//
// Wraps the wasm-bindgen client (bindings-wasm/pkg), re-exports its generated
// swap DTOs, and adds typed helpers for the remaining SDK surface.

import initWasm, {
  BoltzClient as WasmSwapClient,
  BtcLikeTransaction,
  type CorridorCard,
  PreparedLiquidSpend as WasmPreparedLiquidSpend,
  PreparedRgbSpend as WasmPreparedRgbSpend,
  RgbCooperativeRefund as WasmRgbCooperativeRefund,
  SwapScript as WasmSwapScript,
  WasmSwapMasterKey,
  KaleidoSwap as WasmKaleidoSwap,
} from "../vendor/bindings_wasm.js";
import {
  makeCreateKaleidoSwap,
  type SwapSigner,
  type WasmKaleidoSwapCore,
} from "./kaleidoswap.js";

/** URL of the packaged WebAssembly binary. */
export const wasmUrl = new URL(
  "../vendor/bindings_wasm_bg.wasm",
  import.meta.url,
);

// Swap payloads are serde-derived Rust DTOs embedded in the WASM declarations.
// 64-bit response integers are bigint; request amounts accept bigint or safe
// JS numbers. See the generated public types below.
export { BoltzClient as SwapClient } from "../vendor/bindings_wasm.js";

/** Rebuild saved Bitcoin/Liquid transactions with `fromHex(hex, chain)` and
 * rebroadcast the same bytes through their existing `broadcast` method. */
export { BtcLikeTransaction };
export * from "./reconcile.js";

export { resolveLightningAddress } from "../vendor/bindings_wasm.js";

/** Create-time timeout checks against a trusted tip (`esploraTipHeight`), to
 * run before paying a reverse swap's invoice or funding a chain swap. The
 * maker's `height()` is not a trusted tip. */
export {
  validateReverseTimeoutHeadroom,
  validateChainTimeoutHeadroom,
} from "../vendor/bindings_wasm.js";

export type {
  CorridorCard,
  CorridorCardAsset,
  CorridorCardMarket,
} from "../vendor/bindings_wasm.js";

export {
  EsploraDataError,
  esploraFeeEstimates,
  selectEsploraFeeRate,
  esploraTipHeight,
  type EsploraDataOptions,
  type EsploraFeeEstimates,
} from "./chain-data.js";

export type {
  Leaf,
  SwapTree,
  RgbLock,
  SubSwapStates,
  RevSwapStates,
  ChainSwapStates,
  Webhook,
  CreateSubmarineRequest,
  CreateSubmarineResponse,
  InternalHopRef,
  CreateReverseRequest,
  CreateReverseResponse,
  CreateChainRequest,
  ChainSwapDetails,
  CreateChainResponse,
  TransactionResponse,
  TransactionOut,
  SwapEvent,
  SwapPayoutStatus,
  GetSwapResponse,
  ClaimDetails,
  RefundDetails,
  SwapRestoreType,
  SwapRestoreResponse,
  SwapRestoreIndexResponse,
} from "../vendor/bindings_wasm.js";

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
   * Build the client even in a browser context: a page, a worker or an
   * extension. Off by default, and the constructor throws rather than running
   * in a browser — see below. Set this only if you have accepted that every
   * visitor can read the key.
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
 * and the browser, so the check is a runtime one: a document context, a Web,
 * shared or service worker, and a browser extension (`chrome.runtime.id`) are
 * all refused unless you pass `allowBrowser: true`. The key is a permanent
 * organization credential with no origin binding and no per-key rate limit, so a
 * key in a browser bundle is visible to every visitor — who can then attribute
 * their own swaps to, or exhaust the limits of, an organization that is not
 * theirs. Nothing in the bundle can prevent that. Put the key in server-side
 * configuration and talk to the maker from there; for attribution from a
 * browser, an extension or a mobile app, use {@link createKaleidoOrgClient}
 * with the organization's public key instead.
 *
 * Redirects are declined under `fetch` as on a server: the SDK issues every
 * maker request with `redirect: "error"`, so a `3xx` away from the maker fails
 * the call without being followed and no header reaches the host it named.
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

export {
  ATTRIBUTION_HEADER,
  ORG_KEY_HEADER,
  normalizeOrgKey,
} from "./org-key.js";

/** Options for {@link createKaleidoOrgClient}. */
export interface KaleidoOrgClientOptions {
  /** The maker's `/v2` base URL — the only origin the key is sent to. */
  makerUrl: string;
  /**
   * The organization key: the 64-hex x-only public key the organization
   * registered in the KaleidoSwap partner panel.
   */
  orgKey: string;
  /** Per-request timeout in seconds. Omit to leave it to the HTTP stack. */
  timeoutSecs?: number;
}

/**
 * A {@link SwapClient} that attributes the swaps it creates to a partner
 * organization by its public organization key. Works in a page, a Web Worker,
 * an extension service worker, React Native and Node alike.
 *
 * The organization key is public by design: the organization proved it holds
 * the private half when it registered the key, and the private half is used
 * only to claim a revshare payout, on the organization's own machine. So there
 * is nothing in the bundle to steal. The key is a **label, never a
 * credential** — it buys the caller no rate limit, discount or access of its
 * own, which is exactly why copying it is harmless.
 *
 * It goes out in `X-Kaleido-Org-Key` on swap creation only, to `makerUrl`
 * only, which must be `https` unless it is a loopback address. A value that is
 * not a point on the curve is rejected here rather than reaching the maker as
 * `400 org_key_malformed`. A maker with organization keys switched off still
 * creates the swap, unattributed, and the SDK logs a warning.
 *
 * ```ts
 * import { init, createKaleidoOrgClient } from "@kaleidorg/swap-sdk";
 *
 * await init();
 * const client = createKaleidoOrgClient({
 *   makerUrl: "https://maker.signet.kaleidoswap.com/v2",
 *   orgKey: import.meta.env.VITE_KALEIDOSWAP_ORG_KEY,
 * });
 * ```
 *
 * A page on another origin also needs the maker's edge to allow
 * `X-Kaleido-Org-Key` in CORS; an extension's host permissions make that moot.
 */
export function createKaleidoOrgClient(
  options: KaleidoOrgClientOptions,
): WasmSwapClient {
  return WasmSwapClient.forKaleidoOrg(options);
}

// WebSocket swap-status stream. Call `runWsLoop()` WITHOUT awaiting (it runs in
// the background), `await subscribeSwap(id)`, then poll `updates().next()`.
// `next()` resolves with a `SwapStatus` (untyped `any` — Boltz-protocol-defined).
// `await close()` when done: it closes the socket and ends the loop, which
// `free()` cannot do while the loop runs.
export {
  BoltzWsApi as SwapWsApi,
  BoltzWsUpdates as SwapWsUpdates,
} from "../vendor/bindings_wasm.js";

/** Parameters for `SwapScript.constructClaim` / `constructRefund`. */
export interface TxParams {
  /** Trusted, already observed claim lockup. Pins input selection; never broadcasts it. */
  claimLockup?: { chain: "bitcoin" | "liquid"; hex: string; txid: string };
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
   * Cooperative refunds require `swapAuth` against KaleidoSwap maker. Once
   * authorized, they spend with no locktime and do not wait for the timeout.
   */
  cooperative?: boolean;
  /** Per-swap credential from create or signed recovery. Required for maker cooperative refunds. */
  swapAuth?: string;
  /**
   * Confirmations the lockup must have, as seen by the Esplora the params
   * name, before a claim reveals the preimage. Defaults to 1. The WASM core
   * checks this itself (tip height and output status from Esplora), so it
   * holds for every caller of `constructClaim`, not only the reconcile
   * helpers. Refunds ignore it.
   */
  minConfirmations?: number;
  /** Explicitly accept claiming an unconfirmed, replaceable lockup. Default false. */
  allowZeroConf?: boolean;
  /**
   * Blocks of headroom to the swap timeout a claim requires, measured
   * against the Esplora tip. Defaults to 6 on Bitcoin and 60 on Liquid; 0
   * accepts the expiry race.
   */
  claimSafetyMarginBlocks?: number;
  /**
   * Build a claim although an unconfirmed spend of the lockup is already
   * visible: only to fee-bump a stuck claim of your own, whose preimage is
   * already public. A confirmed spend is always refused. Default false.
   */
  allowConflictingSpend?: boolean;
}

/** Claim policy fields, shared with `TxParams`; see there. */
export type ClaimPolicyParams = Pick<
  TxParams,
  | "minConfirmations"
  | "allowZeroConf"
  | "claimSafetyMarginBlocks"
  | "allowConflictingSpend"
>;

/** Parameters for the caller-funded L-USDT PSET prepare methods. */
export interface LiquidPsetParams extends ClaimPolicyParams {
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

/** Bitcoin fee funding for an RGB claim or refund. */
export type RgbSpendFunding =
  { kind: "htlcValue"; feeRateSatVb: bigint } | { kind: "callerInputs" };

/** Parameters for an RGB script-path spend. */
export interface RgbPsbtParams {
  /** Address from the RGB wallet's witness receive; not an arbitrary BTC address. */
  outputAddress: string;
  funding: RgbSpendFunding;
  maxFee: bigint;
  swapId: string;
  makerBaseUrl: string;
  makerTimeoutSecs?: number;
  network: Network;
  bitcoinEsploraUrl: string;
  esploraTimeoutSecs?: number;
  /** Required colored lock transaction; prevents third-party output selection. */
  lockupTxHex: string;
}

/** Base64 unsigned PSBT and the RGB allocation the wallet must construct. */
export interface RgbPsbtTemplate {
  psbt: string;
  swapOutpoint: string;
  swapInputIndex: number;
  commitmentOutputIndex: number;
  paymentOutputIndex: number;
  assetId: string;
  amount: bigint;
  paymentValue: bigint;
  maxFee: bigint;
  requiresFunding: boolean;
}

export interface RgbAllocation {
  assetId: string;
  vout?: number | null;
  amount: bigint;
}

/** PSBT and allocations returned by the caller's RGB wallet after coloring. */
export interface ColoredRgbPsbt {
  psbt: string;
  allocations: RgbAllocation[];
}

export interface FinalizedRgbSpend {
  /** HTLC input finalized; the wallet must still sign any BTC fee inputs. */
  psbt: string;
  swapInputIndex: number;
  /** Present only when every input is final. Free this handle after use. */
  transaction: BtcLikeTransaction | null;
}

/** Immutable spend retained across the wallet funding and coloring steps. */
export interface RgbCooperativeRefundRequest {
  protocol: "rgb-coop-refund-v1";
  psbt: string;
  index: number;
  pubNonce: string;
  sessionId: string;
}

export interface RgbCooperativeRefundResponse {
  sessionId: string;
  requestHash: string;
  pubNonce: string;
  partialSignature: string;
}

/** A one-use signing session. Persist the wallet operation, never this object's secret nonce. */
export class RgbCooperativeRefund {
  constructor(private readonly inner: WasmRgbCooperativeRefund) {}

  request(): RgbCooperativeRefundRequest {
    return this.inner.request() as RgbCooperativeRefundRequest;
  }

  complete(
    response: RgbCooperativeRefundResponse,
    keysSecretHex: string,
  ): FinalizedRgbSpend {
    return this.inner.complete(response, keysSecretHex) as FinalizedRgbSpend;
  }

  free(): void {
    this.inner.free();
  }
}

/** Submit the immutable request with the credential returned at swap creation. */
export async function getRgbRefundPartialSig(
  client: WasmSwapClient,
  swapId: string,
  request: RgbCooperativeRefundRequest,
  swapAuth: string,
): Promise<RgbCooperativeRefundResponse> {
  return (await client.getRgbRefundPartialSig(
    swapId,
    request,
    swapAuth,
  )) as RgbCooperativeRefundResponse;
}

export class PreparedRgbSpend {
  private constructor(private readonly inner: WasmPreparedRgbSpend) {}

  template(): RgbPsbtTemplate {
    return this.inner.template() as RgbPsbtTemplate;
  }

  /** Returns a new spend; keep it for finalization after coloring. */
  fund(fundedPsbt: string): PreparedRgbSpend {
    return PreparedRgbSpend.wrap(this.inner.fund(fundedPsbt));
  }

  beginCooperativeRefund(
    coloredPsbt: ColoredRgbPsbt,
    keysSecretHex: string,
    swapId: string,
  ): RgbCooperativeRefund {
    return new RgbCooperativeRefund(
      this.inner.beginCooperativeRefund(coloredPsbt, keysSecretHex, swapId),
    );
  }

  finalizeClaim(
    coloredPsbt: ColoredRgbPsbt,
    keysSecretHex: string,
    preimageHex: string,
  ): FinalizedRgbSpend {
    return this.inner.finalizeClaim(
      coloredPsbt,
      keysSecretHex,
      preimageHex,
    ) as FinalizedRgbSpend;
  }

  finalizeRefund(
    coloredPsbt: ColoredRgbPsbt,
    keysSecretHex: string,
  ): FinalizedRgbSpend {
    return this.inner.finalizeRefund(
      coloredPsbt,
      keysSecretHex,
    ) as FinalizedRgbSpend;
  }

  free(): void {
    this.inner.free();
  }

  static wrap(inner: WasmPreparedRgbSpend): PreparedRgbSpend {
    return new PreparedRgbSpend(inner);
  }
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
function toWasmParams<T extends TxParams | LiquidPsetParams | RgbPsbtParams>(
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

  /**
   * Rebuild from a `swapRestore()` entry using the public key derived at the
   * selected details' `keyIndex`. Chain swaps require an explicit side.
   * BTC and L-BTC are supported; token restores lack the required asset context.
   */
  static fromRestored(
    restoredSwap: unknown,
    ourPubkeyHex: string,
    network: Network,
    side?: "lockup" | "claim",
  ): SwapScript {
    return new SwapScript(
      WasmSwapScript.fromRestored(restoredSwap, ourPubkeyHex, network, side),
    );
  }

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

  async prepareRgbClaim(params: RgbPsbtParams): Promise<PreparedRgbSpend> {
    return PreparedRgbSpend.wrap(
      await this.inner.prepareRgbClaim(toWasmParams(params)),
    );
  }

  async prepareRgbRefund(params: RgbPsbtParams): Promise<PreparedRgbSpend> {
    return PreparedRgbSpend.wrap(
      await this.inner.prepareRgbRefund(toWasmParams(params)),
    );
  }

  async prepareRgbCooperativeRefund(
    params: RgbPsbtParams,
  ): Promise<PreparedRgbSpend> {
    return PreparedRgbSpend.wrap(
      await this.inner.prepareRgbCooperativeRefund(toWasmParams(params)),
    );
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

  /** Signed discovery card; null when unavailable or withdrawn (404).
   * Does not verify the signature or establish trust in the advertised identity. */
  card(): Promise<CorridorCard | null> {
    return this.client.corridorCard();
  }

  /** Exact unverified v0 card JSON, preserving unknown fields and member order
   * for lossless signature verification; null on 404. */
  async cardRaw(): Promise<string | null> {
    return (await this.client.corridorCardRaw()) ?? null;
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

// ---------------------------------------------------------------------------
// Atomic BTC L1 <-> USDT-RGB L1 swaps — the maker's `/v2/swap/atomic` wire.
//
// One transaction moves both legs: no HTLC and no refund. The SDK quotes,
// verifies and tracks; the wallet's rgb-lib accepts the offer
// (`accept_swap_offer`), signs the proposal (`complete_swap_proposal`) and
// accepts the transfers (`accept_swap_transfers`). rgb-lib messages cross as
// JSON text: their 64-bit fields do not survive a round trip through JS
// numbers or `bigint`.
// ---------------------------------------------------------------------------

/** Send BTC, receive USDT-RGB. */
export const ATOMIC_BTC_TO_USDT_RGB = "BTC/USDT-RGB";
/** Send USDT-RGB, receive BTC. */
export const ATOMIC_USDT_RGB_TO_BTC = "USDT-RGB/BTC";

export type AtomicPairName =
  typeof ATOMIC_BTC_TO_USDT_RGB | typeof ATOMIC_USDT_RGB_TO_BTC;

/** Which amount of a quote request is fixed: what the taker sends or receives. */
export type AtomicAmountDirection = "from" | "to";

export interface AtomicQuoteRequest {
  pair: AtomicPairName;
  /** Sats for BTC, the RGB contract's smallest unit for the asset. */
  amount: bigint;
  direction?: AtomicAmountDirection;
}

/** What the caller expects of a quote, beyond what it asked for. */
/**
 * What the caller expects of a quote, beyond what it asked for.
 *
 * Amounts are in each asset's base units, the ones the quote itself uses:
 * sats for BTC, and the RGB contract's smallest unit for USDT-RGB, which has
 * 6 decimals (1 USDT = `1_000_000n`). Never the pair card's display units.
 */
export interface AtomicQuoteExpectations {
  /** The USDT-RGB contract id the wallet trades. */
  assetId: string;
  /** rgb-lib's `BitcoinNetwork` of the wallet, e.g. `"Regtest"`, `"Signet"`. */
  bitcoinNetwork: string;
  /** RGB proxy the wallet requires, if any. */
  proxyUrl?: string;
  /** Ceiling on the miner fee the taker funds, sats. */
  maxNetworkFeeSat?: bigint;
  /**
   * Ceiling on the fee the maker declares (`serviceFee`), in the output
   * asset's base units. Not price protection: a maker can declare no fee and
   * take its cut in the rate. Only `minToAmount` / `maxFromAmount` bound what
   * the taker actually gets or pays.
   */
  maxServiceFee?: bigint;
  /** For a `from` quote: the least the taker accepts to receive, in the output asset's base units. */
  minToAmount?: bigint;
  /** For a `to` quote: the most the taker accepts to send, in the input asset's base units. */
  maxFromAmount?: bigint;
  /** Longest the offer may stay open, seconds from now. */
  maxOfferTtlSecs?: bigint;
  /**
   * Ceiling on the sats each RGB receive output carries. The taker funds
   * them into its own coloured UTXO; the ceiling stops a maker from tying up
   * an arbitrary amount of the taker's BTC there.
   */
  maxRgbOutputSat?: bigint;
}

/** A coin an rgb-lib swap message spends. */
export interface AtomicOutpoint {
  txid: string;
  vout: number;
}

/** A quote that passed verification. */
export interface AtomicQuote {
  id: string;
  pair: AtomicPairName;
  direction: AtomicAmountDirection;
  /** What the taker sends. */
  fromAmount: bigint;
  /** What the taker receives, net of the maker's fee. */
  toAmount: bigint;
  assetId: string;
  /** Miner fee the taker funds on top of `fromAmount`, sats. */
  networkFeeSat: bigint;
  /** The maker's fee, in the output asset's units. */
  serviceFee: bigint;
  /** Unix seconds: post the request before this. */
  expiresAt: bigint;
  /** Unix seconds: post the completion before this. */
  offerExpiresAt: bigint;
  /** rgb-lib `OnchainSwapOffer` JSON, for the wallet's `accept_swap_offer`. */
  offer: string;
}

export interface AtomicProposal {
  id: string;
  status: string;
  /** rgb-lib `OnchainSwapProposal` JSON, for `complete_swap_proposal`. */
  proposal: string;
  /**
   * The coins the proposal says are the maker's. None of them is one of the
   * `walletOutpoints` passed to `request`, and none repeats.
   */
  makerInputs: AtomicOutpoint[];
}

export interface AtomicCompletion {
  id: string;
  status: string;
  txid: string;
  /** Finalized rgb-lib `OnchainSwapCompletion` JSON, for `accept_swap_transfers`. */
  completion: string;
}

export type AtomicSwapState =
  | "swap.created"
  | "swap.requested"
  | "transaction.proposed"
  | "transaction.mempool"
  | "transaction.confirmed"
  | "transaction.claimed"
  | "swap.expired"
  | "swap.cancelled"
  | "transaction.failed";

/** No update follows these; `transaction.claimed` is the success. */
export const ATOMIC_TERMINAL_STATES: ReadonlySet<AtomicSwapState> = new Set([
  "transaction.claimed",
  "swap.expired",
  "swap.cancelled",
  "transaction.failed",
]);

export interface AtomicSwapStatus {
  id: string;
  pair: AtomicPairName;
  status: AtomicSwapState;
  /** The maker's internal state, for diagnostics. */
  state: string;
  fromAmount: bigint;
  toAmount: bigint;
  assetId: string;
  networkFeeSat: bigint;
  txid?: string | null;
  confirmations: number;
  expiresAt: bigint;
  offerExpiresAt: bigint;
  createdAt: bigint;
}

export interface AtomicPairCard {
  hash: string;
  rate: number;
  limits: { minimal: bigint; maximal: bigint };
  fees: { percentage: number; minerFees: bigint };
}

/** from currency -> to currency -> card; empty while the maker is halted. */
export type AtomicPairs = Record<string, Record<string, AtomicPairCard>>;

export interface AtomicTrackOptions {
  /** Delay between status reads. Default 5 s. */
  intervalMs?: number;
  /** Called with every status read, before the terminal check. */
  onStatus?: (status: AtomicSwapStatus) => void;
  signal?: AbortSignal;
}

/**
 * Typed façade over a {@link SwapClient}'s atomic-swap methods.
 *
 * ```ts
 * const atomic = new AtomicSwaps(SwapClient.forNetwork("regtest"));
 * const quote = await atomic.quote(
 *   { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n },
 *   { assetId: USDT_CONTRACT, bitcoinNetwork: "Regtest", maxNetworkFeeSat: 5_000n },
 * );
 * const request = wallet.acceptSwapOffer(quote.offer);        // rgb-lib
 * const { proposal } = await atomic.request(quote.id, request);
 * const completion = wallet.completeSwapProposal(proposal);   // rgb-lib
 * const done = await atomic.complete(quote.id, completion);
 * const final = await atomic.track(quote.id);
 * ```
 */
export class AtomicSwaps {
  constructor(private readonly client: WasmSwapClient) {}

  pairs(): Promise<AtomicPairs> {
    return this.client.atomicPairs() as Promise<AtomicPairs>;
  }

  /**
   * Quote a swap, then verify it: the quote must answer `request`, match
   * `expectations`, be unexpired at `nowSecs`, and carry an offer whose legs,
   * miner fee and expiry are the quote's, with no platform fee. Rejects with
   * code `Protocol` otherwise; never hand an unverified offer to the wallet.
   */
  quote(
    request: AtomicQuoteRequest,
    expectations: AtomicQuoteExpectations,
    nowSecs: bigint = BigInt(Math.floor(Date.now() / 1000)),
  ): Promise<AtomicQuote> {
    return this.client.atomicQuote(
      { direction: "from", ...request },
      expectations,
      nowSecs,
    ) as Promise<AtomicQuote>;
  }

  /**
   * Post the wallet's `OnchainSwapRequest` JSON; resolves with the maker's
   * unsigned proposal, checked to answer that request.
   *
   * `walletOutpoints` is every coin the wallet holds: rgb-lib's
   * `list_unspents`, as `unspents.map((u) => u.utxo.outpoint)`. It must
   * include the request's own inputs. A proposal that lists any of these
   * coins as the maker's is refused: rgb-lib before kaleidoswap/rgb-lib#18
   * would sign it and hand its value to the maker.
   */
  request(
    swapId: string,
    requestJson: string,
    walletOutpoints: readonly AtomicOutpoint[],
  ): Promise<AtomicProposal> {
    return this.client.atomicRequest(
      swapId,
      requestJson,
      walletOutpoints,
    ) as Promise<AtomicProposal>;
  }

  /** Post the wallet's signed `OnchainSwapCompletion` JSON. The maker signs
   * last and broadcasts; the answer is checked to be the signed transaction. */
  complete(swapId: string, completionJson: string): Promise<AtomicCompletion> {
    return this.client.atomicComplete(
      swapId,
      completionJson,
    ) as Promise<AtomicCompletion>;
  }

  status(swapId: string): Promise<AtomicSwapStatus> {
    return this.client.atomicSwap(swapId) as Promise<AtomicSwapStatus>;
  }

  /** Poll {@link status} until a terminal state, and resolve with it. */
  async track(
    swapId: string,
    { intervalMs = 5_000, onStatus, signal }: AtomicTrackOptions = {},
  ): Promise<AtomicSwapStatus> {
    for (;;) {
      signal?.throwIfAborted();
      const status = await this.status(swapId);
      onStatus?.(status);
      if (ATOMIC_TERMINAL_STATES.has(status.status)) return status;
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(resolve, intervalMs);
        signal?.addEventListener(
          "abort",
          () => {
            clearTimeout(timer);
            reject(signal.reason);
          },
          { once: true },
        );
      });
    }
  }
}

// ---------------------------------------------------------------------------
// KaleidoSwap — the high-level surface. The state machine is the Rust core's
// (`kaleidorg_swap_sdk::kaleidoswap`); see ./kaleidoswap.ts for the types.
// ---------------------------------------------------------------------------

export * from "./kaleidoswap.js";

/**
 * The high-level entry: `quote`, `start`, `resume` over an injected store and
 * signer. Runs `init()` itself.
 */
export const createKaleidoSwap = makeCreateKaleidoSwap({
  init: () => init(),
  KaleidoSwap: WasmKaleidoSwap as unknown as WasmKaleidoSwapCore["KaleidoSwap"],
});

/** The wasm class behind {@link createKaleidoSwap}, for the Node entry to bind to its own `init`. */
export const wasmKaleidoSwapClass =
  WasmKaleidoSwap as unknown as WasmKaleidoSwapCore["KaleidoSwap"];

/**
 * A {@link SwapSigner} over a swap mnemonic (`SwapMasterKey.fromSwapMnemonic`).
 * The master key is built on first use, after `init()` has run.
 */
export function mnemonicSigner(
  mnemonic: string,
  network: Network,
  passphrase?: string,
): SwapSigner {
  let master: SwapMasterKey | undefined;
  const get = () =>
    (master ??= SwapMasterKey.fromSwapMnemonic(mnemonic, network, passphrase));
  return {
    deriveKey: (index) => get().deriveSwapKey(BigInt(index)),
    derivePreimage: (index) => get().derivePreimage(BigInt(index)),
  };
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
  /**
   * Discover swaps using this account's xpub and direct child keys (`m/index`).
   * The maker's default path is for a root xpub and would miss these keys.
   * This is read-only discovery; recover each swap's authorization separately.
   */
  restore(
    client: WasmSwapClient,
    gapLimit?: number,
  ): ReturnType<WasmSwapClient["swapRestore"]> {
    return client.swapRestore(this.masterXpub(), "m", gapLimit);
  }
  /**
   * Find the highest known swap-key index using the same path as {@link restore}.
   * A scan is bounded by its gap limit; retain the wallet's local high-water
   * mark as well rather than treating an empty scan as permission to reuse keys.
   */
  restoreIndex(
    client: WasmSwapClient,
    gapLimit?: number,
  ): ReturnType<WasmSwapClient["swapRestoreIndex"]> {
    return client.swapRestoreIndex(this.masterXpub(), "m", gapLimit);
  }
  deriveSwapKey(index: bigint): DerivedKey {
    return this.inner.deriveSwapKey(index);
  }
  derivePreimage(index: bigint): DerivedPreimage {
    return this.inner.derivePreimage(index);
  }
}
