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
