// KaleidoSwap SDK — TypeScript surface.
//
// Wraps the wasm-bindgen client (bindings-wasm/pkg) with typed signatures drawn
// from the openapi-typescript models in ./generated/node-types.ts. The wasm
// boundary passes plain JS objects (typed `any`); this layer restores the domain
// types — the browser analogue of the Python pydantic boundary. The single
// source of truth is specs/rgb-lightning-node.yaml (also feeds the Rust typify
// types and the Python pydantic models).

import initWasm, {
  RlnClient as WasmRlnClient,
  WasmSwapMasterKey,
} from "../../bindings-wasm/pkg/bindings_wasm";
import type { components } from "./generated/node-types";

// Boltz swap API client. Re-exported from the wasm module as-is: its request/
// response payloads are currently untyped (`any`) because the Boltz swap DTOs are
// Rust-defined and have no OpenAPI spec to generate TS types from. A typed
// surface would need a schema-generation step (schemars) or hand-written types.
export { BoltzClient } from "../../bindings-wasm/pkg/bindings_wasm";

// Client-side swap-script + claim/refund transaction construction. Re-exported
// as opaque handles; `SwapScript.constructClaim/constructRefund` take a
// `TxParams` object (below) and return a `BtcLikeTransaction`.
export { SwapScript, BtcLikeTransaction } from "../../bindings-wasm/pkg/bindings_wasm";

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
  /** Cooperative (MuSig2 keyspend) claim/refund. Defaults to true. */
  cooperative?: boolean;
}

/** Domain models generated from the RLN OpenAPI spec. */
export type Schemas = components["schemas"];
export type { components } from "./generated/node-types";

/**
 * Load and instantiate the wasm module. Call once (await it) before creating any
 * client. `input` is an optional URL/Request/Response/BufferSource for the .wasm.
 */
export async function init(input?: Parameters<typeof initWasm>[0]): Promise<void> {
  await initWasm(input);
}

/** Typed async client for a single RGB Lightning Node. */
export class RlnClient {
  private constructor(private readonly inner: WasmRlnClient) {}

  /** Create a client. `init()` must have resolved first. */
  static connect(baseUrl: string, token?: string, timeoutSecs?: bigint): RlnClient {
    return new RlnClient(new WasmRlnClient(baseUrl, token, timeoutSecs));
  }

  setToken(token?: string): void {
    this.inner.setToken(token);
  }

  // Node lifecycle & info
  init(req: Schemas["InitRequest"]): Promise<Schemas["InitResponse"]> {
    return this.inner.init(req);
  }
  unlock(req: Schemas["UnlockRequest"]): Promise<void> {
    return this.inner.unlock(req);
  }
  lock(): Promise<void> {
    return this.inner.lock();
  }
  nodeInfo(): Promise<Schemas["NodeInfoResponse"]> {
    return this.inner.nodeInfo();
  }
  networkInfo(): Promise<Schemas["NetworkInfoResponse"]> {
    return this.inner.networkInfo();
  }
  address(): Promise<Schemas["AddressResponse"]> {
    return this.inner.address();
  }

  // Invoices
  lnInvoice(req: Schemas["LNInvoiceRequest"]): Promise<Schemas["LNInvoiceResponse"]> {
    return this.inner.lnInvoice(req);
  }
  decodeLnInvoice(
    req: Schemas["DecodeLNInvoiceRequest"],
  ): Promise<Schemas["DecodeLNInvoiceResponse"]> {
    return this.inner.decodeLnInvoice(req);
  }
  invoiceStatus(
    req: Schemas["InvoiceStatusRequest"],
  ): Promise<Schemas["InvoiceStatusResponse"]> {
    return this.inner.invoiceStatus(req);
  }

  // Payments
  sendPayment(req: Schemas["SendPaymentRequest"]): Promise<Schemas["SendPaymentResponse"]> {
    return this.inner.sendPayment(req);
  }
  getPayment(req: Schemas["GetPaymentRequest"]): Promise<Schemas["GetPaymentResponse"]> {
    return this.inner.getPayment(req);
  }
  listPayments(): Promise<Schemas["ListPaymentsResponse"]> {
    return this.inner.listPayments();
  }
  keysend(req: Schemas["KeysendRequest"]): Promise<Schemas["KeysendResponse"]> {
    return this.inner.keysend(req);
  }

  // RGB
  rgbInvoice(req: Schemas["RgbInvoiceRequest"]): Promise<Schemas["RgbInvoiceResponse"]> {
    return this.inner.rgbInvoice(req);
  }
  decodeRgbInvoice(
    req: Schemas["DecodeRGBInvoiceRequest"],
  ): Promise<Schemas["DecodeRGBInvoiceResponse"]> {
    return this.inner.decodeRgbInvoice(req);
  }
  listAssets(req: Schemas["ListAssetsRequest"]): Promise<Schemas["ListAssetsResponse"]> {
    return this.inner.listAssets(req);
  }
  assetBalance(
    req: Schemas["AssetBalanceRequest"],
  ): Promise<Schemas["AssetBalanceResponse"]> {
    return this.inner.assetBalance(req);
  }
  sendRgb(req: Schemas["SendRgbRequest"]): Promise<Schemas["SendRgbResponse"]> {
    return this.inner.sendRgb(req);
  }

  // Channels & peers
  listChannels(): Promise<Schemas["ListChannelsResponse"]> {
    return this.inner.listChannels();
  }
  openChannel(req: Schemas["OpenChannelRequest"]): Promise<Schemas["OpenChannelResponse"]> {
    return this.inner.openChannel(req);
  }
  closeChannel(req: Schemas["CloseChannelRequest"]): Promise<void> {
    return this.inner.closeChannel(req);
  }
  connectPeer(req: Schemas["ConnectPeerRequest"]): Promise<void> {
    return this.inner.connectPeer(req);
  }

  // Swaps (maker / taker)
  makerInit(req: Schemas["MakerInitRequest"]): Promise<Schemas["MakerInitResponse"]> {
    return this.inner.makerInit(req);
  }
  makerExecute(req: Schemas["MakerExecuteRequest"]): Promise<void> {
    return this.inner.makerExecute(req);
  }
  taker(req: Schemas["TakerRequest"]): Promise<void> {
    return this.inner.taker(req);
  }
  getSwap(req: Schemas["GetSwapRequest"]): Promise<Schemas["GetSwapResponse"]> {
    return this.inner.getSwap(req);
  }
  listSwaps(): Promise<Schemas["ListSwapsResponse"]> {
    return this.inner.listSwaps();
  }
  decodeSwapstring(
    req: Schemas["DecodeSwapstringRequest"],
  ): Promise<Schemas["DecodeSwapstringResponse"]> {
    return this.inner.decodeSwapstring(req);
  }
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

export type Network = "mainnet" | "testnet" | "regtest";

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
