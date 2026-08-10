// KaleidoSwap swap SDK — TypeScript surface.
//
// Wraps the wasm-bindgen client (bindings-wasm/pkg) with typed signatures. The
// wasm boundary passes plain JS objects (typed `any`); this layer restores the
// domain types with hand-written interfaces.

import initWasm, {
  BtcLikeTransaction,
  PreparedLiquidSpend as WasmPreparedLiquidSpend,
  SwapScript as WasmSwapScript,
  WasmSwapMasterKey,
} from "../vendor/bindings_wasm.js";
import type { InitInput } from "../vendor/bindings_wasm.js";

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
  /**
   * Refund-side key secret (hex), for `constructCooperativeClaim` only.
   *
   * That path partial-signs a temporary refund against the lockup script, so it
   * needs the swap's **refund** key — not necessarily `keysSecretHex`, which is
   * the claim key. Defaults to `keysSecretHex`, which is correct when the swap
   * was created with one key for both sides (as `SwapMasterKey`-derived swaps
   * are). Ignored by every other method.
   */
  refundKeysSecretHex?: string;
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

/** Stable error shape thrown by core swap operations across the WASM boundary. */
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
   * The witness is far smaller than the script path's, so pass an absolute fee
   * (`feeAbsoluteSat`) sized for a keyspend rather than a rate meant for a
   * script spend.
   *
   * Falls back to a non-cooperative claim when the server has already claimed
   * and no longer offers details to sign against.
   *
   * Set `params.refundKeysSecretHex` if the swap was created with distinct claim
   * and refund keys — the partial signature is made with the refund key.
   */
  constructCooperativeClaim(
    preimageHex: string,
    params: TxParams,
    lockupScript: SwapScript,
  ): Promise<BtcLikeTransaction> {
    return this.inner.constructCooperativeClaim(
      preimageHex,
      params,
      lockupScript.inner,
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
 * Load and instantiate the wasm module. Call once (await it) before creating any
 * client. Browsers can omit `input`. Node consumers must read {@link wasmUrl}
 * and pass its bytes because Node's `fetch` does not load `file:` URLs.
 */
export async function init(
  input?: InitInput | Promise<InitInput>,
): Promise<void> {
  await initWasm(input === undefined ? undefined : { module_or_path: input });
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
