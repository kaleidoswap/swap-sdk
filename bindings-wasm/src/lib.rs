//! WebAssembly bindings for the KaleidoSwap SDK.
//!
//! Exposes the RGB Lightning Node client and the swap key-management surface to
//! JavaScript/TypeScript via `wasm-bindgen`. Request/response values cross to JS
//! as plain objects through `serde-wasm-bindgen`; on the TS side they are typed
//! with the openapi-typescript models (`node-types.ts`) — the browser analogue
//! of the Python pydantic boundary.
//!
//! Build with `wasm-pack build` (see `make wasm-pack-build`), which emits a JS
//! package with generated `.d.ts` under `bindings-wasm/pkg/`.

use wasm_bindgen::prelude::*;

fn js_err<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn to_js<T: serde::Serialize>(v: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(v).map_err(js_err)
}

fn from_js<T: serde::de::DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(js_err)
}

/// Async client for a single RGB Lightning Node, exposed to JS.
///
/// Methods that take a request accept a plain JS object matching the
/// corresponding `node-types.ts` type; methods return the parsed response as a
/// JS object (or `void`). Errors reject the Promise with the error message.
#[wasm_bindgen]
pub struct RlnClient {
    inner: rln_client::RlnClient,
}

#[wasm_bindgen]
impl RlnClient {
    /// `new RlnClient(baseUrl, token?, timeoutSecs?)`
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: String, token: Option<String>, timeout_secs: Option<u64>) -> RlnClient {
        RlnClient {
            inner: rln_client::RlnClient::new(
                base_url,
                token,
                timeout_secs.map(std::time::Duration::from_secs),
            ),
        }
    }

    #[wasm_bindgen(js_name = setToken)]
    pub fn set_token(&mut self, token: Option<String>) {
        self.inner.set_token(token);
    }

    // ---- Node lifecycle & info --------------------------------------------

    pub async fn init(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.init(from_js(req)?).await.map_err(js_err)?)
    }

    pub async fn unlock(&self, req: JsValue) -> Result<(), JsValue> {
        self.inner.unlock(from_js(req)?).await.map_err(js_err)
    }

    pub async fn lock(&self) -> Result<(), JsValue> {
        self.inner.lock().await.map_err(js_err)
    }

    #[wasm_bindgen(js_name = nodeInfo)]
    pub async fn node_info(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.node_info().await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = networkInfo)]
    pub async fn network_info(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.network_info().await.map_err(js_err)?)
    }

    pub async fn address(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.address().await.map_err(js_err)?)
    }

    // ---- Invoices ----------------------------------------------------------

    #[wasm_bindgen(js_name = lnInvoice)]
    pub async fn ln_invoice(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.ln_invoice(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = decodeLnInvoice)]
    pub async fn decode_ln_invoice(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.decode_ln_invoice(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = invoiceStatus)]
    pub async fn invoice_status(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.invoice_status(from_js(req)?).await.map_err(js_err)?)
    }

    // ---- Payments ----------------------------------------------------------

    #[wasm_bindgen(js_name = sendPayment)]
    pub async fn send_payment(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.send_payment(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = getPayment)]
    pub async fn get_payment(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_payment(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = listPayments)]
    pub async fn list_payments(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.list_payments().await.map_err(js_err)?)
    }

    pub async fn keysend(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.keysend(from_js(req)?).await.map_err(js_err)?)
    }

    // ---- RGB ---------------------------------------------------------------

    #[wasm_bindgen(js_name = rgbInvoice)]
    pub async fn rgb_invoice(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.rgb_invoice(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = decodeRgbInvoice)]
    pub async fn decode_rgb_invoice(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.decode_rgb_invoice(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = listAssets)]
    pub async fn list_assets(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.list_assets(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = assetBalance)]
    pub async fn asset_balance(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.asset_balance(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = sendRgb)]
    pub async fn send_rgb(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.send_rgb(from_js(req)?).await.map_err(js_err)?)
    }

    // ---- Channels & peers --------------------------------------------------

    #[wasm_bindgen(js_name = listChannels)]
    pub async fn list_channels(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.list_channels().await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = openChannel)]
    pub async fn open_channel(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.open_channel(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = closeChannel)]
    pub async fn close_channel(&self, req: JsValue) -> Result<(), JsValue> {
        self.inner.close_channel(from_js(req)?).await.map_err(js_err)
    }

    #[wasm_bindgen(js_name = connectPeer)]
    pub async fn connect_peer(&self, req: JsValue) -> Result<(), JsValue> {
        self.inner.connect_peer(from_js(req)?).await.map_err(js_err)
    }

    // ---- Swaps (maker / taker) ---------------------------------------------

    #[wasm_bindgen(js_name = makerInit)]
    pub async fn maker_init(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.maker_init(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = makerExecute)]
    pub async fn maker_execute(&self, req: JsValue) -> Result<(), JsValue> {
        self.inner.maker_execute(from_js(req)?).await.map_err(js_err)
    }

    pub async fn taker(&self, req: JsValue) -> Result<(), JsValue> {
        self.inner.taker(from_js(req)?).await.map_err(js_err)
    }

    #[wasm_bindgen(js_name = getSwap)]
    pub async fn get_swap(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_swap(from_js(req)?).await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = listSwaps)]
    pub async fn list_swaps(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.list_swaps().await.map_err(js_err)?)
    }

    #[wasm_bindgen(js_name = decodeSwapstring)]
    pub async fn decode_swapstring(&self, req: JsValue) -> Result<JsValue, JsValue> {
        to_js(&self.inner.decode_swapstring(from_js(req)?).await.map_err(js_err)?)
    }
}

// ============================================================================
// Swap key management (client-side crypto). This is the browser entry point for
// deriving per-swap keys and preimages; swap-script/tx construction is exposed
// incrementally on top of these.
// ============================================================================

use kaleidoswap_sdk::network::Network;
use kaleidoswap_sdk::util::secrets::{Preimage, SwapMasterKey};

/// A derived swap key, returned to JS as `{ publicKey, secretKey }` (hex).
#[derive(serde::Serialize)]
struct DerivedKey {
    #[serde(rename = "publicKey")]
    public_key: String,
    #[serde(rename = "secretKey")]
    secret_key: String,
}

/// Wraps [`SwapMasterKey`] for JS: BIP85-derived swap keys from a wallet mnemonic.
#[wasm_bindgen]
pub struct WasmSwapMasterKey {
    inner: SwapMasterKey,
}

#[wasm_bindgen]
impl WasmSwapMasterKey {
    /// Derive the swap master key from a wallet mnemonic (BIP85 index 26589).
    /// `network` is one of "mainnet" | "testnet" | "regtest".
    #[wasm_bindgen(js_name = fromWalletMnemonic)]
    pub fn from_wallet_mnemonic(
        wallet_mnemonic: String,
        passphrase: Option<String>,
        network: String,
    ) -> Result<WasmSwapMasterKey, JsValue> {
        let inner = SwapMasterKey::new(&wallet_mnemonic, passphrase.as_deref(), parse_network(&network)?)
            .map_err(js_err)?;
        Ok(WasmSwapMasterKey { inner })
    }

    /// Reconstruct from the swap (rescue) mnemonic directly.
    #[wasm_bindgen(js_name = fromSwapMnemonic)]
    pub fn from_swap_mnemonic(
        mnemonic: String,
        passphrase: Option<String>,
        network: String,
    ) -> Result<WasmSwapMasterKey, JsValue> {
        let inner = SwapMasterKey::from_mnemonic(&mnemonic, passphrase.as_deref(), parse_network(&network)?)
            .map_err(js_err)?;
        Ok(WasmSwapMasterKey { inner })
    }

    /// The BIP85-derived swap (rescue) mnemonic.
    #[wasm_bindgen(js_name = swapMnemonic)]
    pub fn swap_mnemonic(&self) -> String {
        self.inner.mnemonic.to_string()
    }

    /// The master xpub to register with the swap-restore API.
    #[wasm_bindgen(js_name = masterXpub)]
    pub fn master_xpub(&self) -> String {
        self.inner.get_master_xpub().to_string()
    }

    /// Derive the swap keypair at `index`, returned as `{ publicKey, secretKey }`.
    #[wasm_bindgen(js_name = deriveSwapKey)]
    pub fn derive_swap_key(&self, index: u64) -> Result<JsValue, JsValue> {
        let kp = self.inner.derive_swapkey(index).map_err(js_err)?;
        to_js(&DerivedKey {
            public_key: kp.public_key().to_string(),
            secret_key: kp.secret_key().display_secret().to_string(),
        })
    }

    /// Derive the deterministic preimage for the swap at `index`
    /// (`sha256(privateKey)`), returned as `{ preimage, sha256, hash160 }` hex.
    #[wasm_bindgen(js_name = derivePreimage)]
    pub fn derive_preimage(&self, index: u64) -> Result<JsValue, JsValue> {
        let kp = self.inner.derive_swapkey(index).map_err(js_err)?;
        let p = Preimage::from_swap_key(&kp);
        to_js(&serde_json::json!({
            "preimage": p.to_string(),
            "sha256": p.sha256.to_string(),
            "hash160": p.hash160.to_string(),
        }))
    }
}

fn parse_network(s: &str) -> Result<Network, JsValue> {
    match s.to_lowercase().as_str() {
        "mainnet" => Ok(Network::Mainnet),
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        other => Err(JsValue::from_str(&format!("unknown network: {other}"))),
    }
}

// ============================================================================
// Boltz swap API client — the taker's query / create / status surface.
//
// Structurally identical to RlnClient (async reqwest + serde DTOs). Note: unlike
// the RLN types, the Boltz swap DTOs are Rust-defined (no OpenAPI spec), so their
// TS types are currently `any` on the JS side — a typed TS surface for these
// would need a schema-generation step (e.g. schemars) or hand-written interfaces.
// ============================================================================

use kaleidoswap_sdk::boltz::{
    BoltzApiClientV2, CreateChainRequest, CreateReverseRequest, CreateSubmarineRequest,
};

fn core_err(e: kaleidoswap_sdk::error::Error) -> JsValue {
    JsValue::from_str(&e.message())
}

/// Async client for the Boltz swap API.
#[wasm_bindgen]
pub struct BoltzClient {
    inner: BoltzApiClientV2,
}

#[wasm_bindgen]
impl BoltzClient {
    /// `new BoltzClient(baseUrl, timeoutSecs?)`
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: String, timeout_secs: Option<u64>) -> BoltzClient {
        BoltzClient {
            inner: BoltzApiClientV2::new(base_url, timeout_secs.map(std::time::Duration::from_secs)),
        }
    }

    /// Client pointed at the default Boltz endpoint for a network
    /// ("mainnet" | "testnet" | "regtest").
    #[wasm_bindgen(js_name = forNetwork)]
    pub fn for_network(network: String) -> Result<BoltzClient, JsValue> {
        Ok(BoltzClient {
            inner: BoltzApiClientV2::default(parse_network(&network)?),
        })
    }

    // ---- Rates / limits ----------------------------------------------------

    #[wasm_bindgen(js_name = feeEstimation)]
    pub async fn fee_estimation(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_fee_estimation().await.map_err(core_err)?)
    }
    pub async fn height(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_height().await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = submarinePairs)]
    pub async fn submarine_pairs(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_submarine_pairs().await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = reversePairs)]
    pub async fn reverse_pairs(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_reverse_pairs().await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = chainPairs)]
    pub async fn chain_pairs(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_chain_pairs().await.map_err(core_err)?)
    }

    // ---- Create swaps ------------------------------------------------------

    #[wasm_bindgen(js_name = createSubmarineSwap)]
    pub async fn create_submarine_swap(&self, req: JsValue) -> Result<JsValue, JsValue> {
        let req: CreateSubmarineRequest = from_js(req)?;
        to_js(&self.inner.post_swap_req(&req).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = createReverseSwap)]
    pub async fn create_reverse_swap(&self, req: JsValue) -> Result<JsValue, JsValue> {
        let req: CreateReverseRequest = from_js(req)?;
        to_js(&self.inner.post_reverse_req(req).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = createChainSwap)]
    pub async fn create_chain_swap(&self, req: JsValue) -> Result<JsValue, JsValue> {
        let req: CreateChainRequest = from_js(req)?;
        to_js(&self.inner.post_chain_req(req).await.map_err(core_err)?)
    }

    // ---- Status / lookups --------------------------------------------------

    #[wasm_bindgen(js_name = submarineTx)]
    pub async fn submarine_tx(&self, id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_submarine_tx(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = reverseTx)]
    pub async fn reverse_tx(&self, id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_reverse_tx(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = chainTxs)]
    pub async fn chain_txs(&self, id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_chain_txs(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = submarinePreimage)]
    pub async fn submarine_preimage(&self, id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_submarine_preimage(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = mrhBip21)]
    pub async fn mrh_bip21(&self, invoice: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_mrh_bip21(&invoice).await.map_err(core_err)?)
    }
    pub async fn swap(&self, swap_id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_swap(&swap_id).await.map_err(core_err)?)
    }
    pub async fn quote(&self, swap_id: String) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_quote(&swap_id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = acceptQuote)]
    pub async fn accept_quote(&self, swap_id: String, amount_sat: u64) -> Result<(), JsValue> {
        self.inner.accept_quote(&swap_id, amount_sat).await.map_err(core_err)
    }
    pub async fn nodes(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.get_nodes().await.map_err(core_err)?)
    }

    // ---- Recovery ----------------------------------------------------------

    #[wasm_bindgen(js_name = swapRestore)]
    pub async fn swap_restore(
        &self,
        xpub: String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        to_js(&self.inner.post_swap_restore(&xpub, derivation_path, gap_limit).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = swapRestoreIndex)]
    pub async fn swap_restore_index(
        &self,
        xpub: String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        to_js(&self.inner.post_swap_restore_index(&xpub, derivation_path, gap_limit).await.map_err(core_err)?)
    }
}
