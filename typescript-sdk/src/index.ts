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
} from "../vendor/bindings_wasm.js";
import type { components } from "./generated/node-types";

// Boltz swap API client. Re-exported from the wasm module as-is: its request/
// response payloads are currently untyped (`any`) because the Boltz swap DTOs are
// Rust-defined and have no OpenAPI spec to generate TS types from. A typed
// surface would need a schema-generation step (schemars) or hand-written types.
// NOTE: 64-bit integer fields in its responses arrive as `bigint` (same lossless
// boundary as the RLN types).
export { BoltzClient } from "../vendor/bindings_wasm.js";

// Client-side swap-script + claim/refund transaction construction. Re-exported
// as opaque handles; their plain-object boundary types are documented below.
export {
  SwapScript,
  PreparedLiquidSpend,
  BtcLikeTransaction,
} from "../vendor/bindings_wasm.js";

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
   * Set `false` for **chain-swap claims** — the cooperative chain path needs the
   * counterparty lockup script + refund keys, which this object does not carry
   * (submarine/reverse cooperative claims work with the default).
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
 * Domain models generated from the RLN OpenAPI spec.
 *
 * Integer fields are `bigint`: the wasm boundary serializes Rust i64/u64 as
 * BigInt so u64 amounts (e.g. RGB asset amounts up to u64::MAX) never lose
 * precision through an f64. Use bigint literals in requests (`1000n`) and
 * {@link toJson} when stringifying responses.
 */
export type Schemas = components["schemas"];
export type { components } from "./generated/node-types";

/**
 * `JSON.stringify` that encodes `bigint` values as decimal strings — plain
 * `JSON.stringify` throws on BigInt. Use for logging/persisting SDK responses.
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

  // ---- Node lifecycle: backup / restore / password / shutdown ------------
  backup(req: Schemas["BackupRequest"]): Promise<void> {
    return this.inner.backup(req);
  }
  restore(req: Schemas["RestoreRequest"]): Promise<void> {
    return this.inner.restore(req);
  }
  changePassword(req: Schemas["ChangePasswordRequest"]): Promise<void> {
    return this.inner.changePassword(req);
  }
  shutdown(): Promise<void> {
    return this.inner.shutdown();
  }

  // ---- BTC on-chain ------------------------------------------------------
  btcBalance(req: Schemas["BtcBalanceRequest"]): Promise<Schemas["BtcBalanceResponse"]> {
    return this.inner.btcBalance(req);
  }
  sendBtc(req: Schemas["SendBtcRequest"]): Promise<Schemas["SendBtcResponse"]> {
    return this.inner.sendBtc(req);
  }
  listTransactions(
    req: Schemas["ListTransactionsRequest"],
  ): Promise<Schemas["ListTransactionsResponse"]> {
    return this.inner.listTransactions(req);
  }
  listUnspents(
    req: Schemas["ListUnspentsRequest"],
  ): Promise<Schemas["ListUnspentsResponse"]> {
    return this.inner.listUnspents(req);
  }
  createUtxos(req: Schemas["CreateUtxosRequest"]): Promise<void> {
    return this.inner.createUtxos(req);
  }
  estimateFee(req: Schemas["EstimateFeeRequest"]): Promise<Schemas["EstimateFeeResponse"]> {
    return this.inner.estimateFee(req);
  }

  // ---- RGB assets: issuance, inflation, metadata & media -----------------
  issueAssetNia(
    req: Schemas["IssueAssetNIARequest"],
  ): Promise<Schemas["IssueAssetNIAResponse"]> {
    return this.inner.issueAssetNia(req);
  }
  issueAssetCfa(
    req: Schemas["IssueAssetCFARequest"],
  ): Promise<Schemas["IssueAssetCFAResponse"]> {
    return this.inner.issueAssetCfa(req);
  }
  issueAssetUda(
    req: Schemas["IssueAssetUDARequest"],
  ): Promise<Schemas["IssueAssetUDAResponse"]> {
    return this.inner.issueAssetUda(req);
  }
  issueAssetIfa(
    req: Schemas["IssueAssetIFARequest"],
  ): Promise<Schemas["IssueAssetIFAResponse"]> {
    return this.inner.issueAssetIfa(req);
  }
  inflate(req: Schemas["InflateRequest"]): Promise<Schemas["InflateResponse"]> {
    return this.inner.inflate(req);
  }
  assetMetadata(
    req: Schemas["AssetMetadataRequest"],
  ): Promise<Schemas["AssetMetadataResponse"]> {
    return this.inner.assetMetadata(req);
  }
  getAssetMedia(
    req: Schemas["GetAssetMediaRequest"],
  ): Promise<Schemas["GetAssetMediaResponse"]> {
    return this.inner.getAssetMedia(req);
  }
  /** Upload asset media bytes, returning its digest. `fileName` defaults to `"media"`. */
  postAssetMedia(
    fileBytes: Uint8Array,
    fileName?: string,
  ): Promise<Schemas["PostAssetMediaResponse"]> {
    return this.inner.postAssetMedia(fileBytes, fileName);
  }

  // ---- RGB transfers -----------------------------------------------------
  listTransfers(
    req: Schemas["ListTransfersRequest"],
  ): Promise<Schemas["ListTransfersResponse"]> {
    return this.inner.listTransfers(req);
  }
  refreshTransfers(req: Schemas["RefreshRequest"]): Promise<void> {
    return this.inner.refreshTransfers(req);
  }
  failTransfers(
    req: Schemas["FailTransfersRequest"],
  ): Promise<Schemas["FailTransfersResponse"]> {
    return this.inner.failTransfers(req);
  }
  sync(req: Schemas["SyncRequest"]): Promise<void> {
    return this.inner.sync(req);
  }

  // ---- Peers & channels (extended) ---------------------------------------
  listPeers(): Promise<Schemas["ListPeersResponse"]> {
    return this.inner.listPeers();
  }
  disconnectPeer(req: Schemas["DisconnectPeerRequest"]): Promise<void> {
    return this.inner.disconnectPeer(req);
  }
  getChannelId(
    req: Schemas["GetChannelIdRequest"],
  ): Promise<Schemas["GetChannelIdResponse"]> {
    return this.inner.getChannelId(req);
  }

  // ---- Utility -----------------------------------------------------------
  signMessage(
    req: Schemas["SignMessageRequest"],
  ): Promise<Schemas["SignMessageResponse"]> {
    return this.inner.signMessage(req);
  }
  sendOnionMessage(req: Schemas["SendOnionMessageRequest"]): Promise<void> {
    return this.inner.sendOnionMessage(req);
  }
  checkIndexerUrl(
    req: Schemas["CheckIndexerUrlRequest"],
  ): Promise<Schemas["CheckIndexerUrlResponse"]> {
    return this.inner.checkIndexerUrl(req);
  }
  checkProxyEndpoint(req: Schemas["CheckProxyEndpointRequest"]): Promise<void> {
    return this.inner.checkProxyEndpoint(req);
  }
  revokeToken(req: Schemas["RevokeTokenRequest"]): Promise<void> {
    return this.inner.revokeToken(req);
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
