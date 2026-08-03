//! WebAssembly bindings for the KaleidoSwap SDK.
//!
//! Exposes the Boltz swap surface and the swap key-management surface to
//! JavaScript/TypeScript via `wasm-bindgen`. Request/response values cross to JS
//! as plain objects through `serde-wasm-bindgen`; the TS SDK adds hand-written
//! types on top of that boundary.
//!
//! Build with `wasm-pack build` (see `make wasm-pack-build`), which emits a JS
//! package with generated `.d.ts` under `bindings-wasm/pkg/`.

use std::sync::Arc;
use wasm_bindgen::prelude::*;

fn js_err<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn to_js<T: serde::Serialize>(v: &T) -> Result<JsValue, JsValue> {
    // Serialize 64-bit integers as JS BigInt so u64 amounts cross the boundary
    // losslessly instead of being rounded through an f64. This matches
    // wasm-bindgen's own u64 <-> bigint mapping in direct signatures;
    // deserialization (from_js) accepts both Number and BigInt, so request
    // objects may use either.
    let ser = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
    v.serialize(&ser).map_err(js_err)
}

fn from_js<T: serde::de::DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(js_err)
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
        let inner = SwapMasterKey::new(
            &wallet_mnemonic,
            passphrase.as_deref(),
            parse_network(&network)?,
        )
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
        let inner = SwapMasterKey::from_mnemonic(
            &mnemonic,
            passphrase.as_deref(),
            parse_network(&network)?,
        )
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
        "signet" => Ok(Network::Signet),
        "regtest" => Ok(Network::Regtest),
        other => Err(JsValue::from_str(&format!("unknown network: {other}"))),
    }
}

// ============================================================================
// Boltz swap API client — the taker's query / create / status surface.
//
// Async reqwest + serde DTOs. Note: the Boltz swap DTOs are Rust-defined (no
// OpenAPI spec), so their TS types are currently `any` on the JS side — a typed
// TS surface for these would need a schema-generation step (e.g. schemars) or
// hand-written interfaces.
// ============================================================================

use kaleidoswap_sdk::boltz::{
    BoltzApiClientV2, CreateChainRequest, CreateReverseRequest, CreateSubmarineRequest,
};

fn core_err(e: kaleidoswap_sdk::error::Error) -> JsValue {
    let error = js_sys::Error::new(&e.message());
    error.set_name(&e.name());
    let _ = js_sys::Reflect::set(
        error.as_ref(),
        &JsValue::from_str("code"),
        &JsValue::from_str(&e.name()),
    );
    error.into()
}

/// Resolve a Boltz asset identifier to its chain and currency before posting a
/// create request, so an unsupported asset cannot leave an orphan server swap.
fn asset_from_boltz(
    s: &str,
    network: &str,
) -> Result<
    (
        kaleidoswap_sdk::network::Chain,
        kaleidoswap_sdk::network::Currency,
    ),
    JsValue,
> {
    use kaleidoswap_sdk::network::{Chain, Currency};

    let net = parse_network(network)?;
    match s {
        "BTC" => Ok((Chain::Bitcoin(net.into()), Currency::Btc)),
        "L-BTC" => Ok((Chain::Liquid(net.into()), Currency::LBtc)),
        "L-USDT" => Ok((Chain::Liquid(net.into()), Currency::LUsdt)),
        other => Err(JsValue::from_str(&format!(
            "unsupported Boltz asset '{other}'"
        ))),
    }
}

#[cfg(test)]
mod boltz_asset_tests {
    use super::*;
    use kaleidoswap_sdk::network::{BitcoinChain, Chain, Currency, LiquidChain};

    #[test]
    fn lusdt_resolves_to_liquid_chain_and_distinct_currency() {
        let (chain, currency) = asset_from_boltz("L-USDT", "regtest").unwrap();

        assert_eq!(chain, Chain::Liquid(LiquidChain::LiquidRegtest));
        assert_eq!(currency, Currency::LUsdt);
    }

    /// `"signet"` must parse (it is the KaleidoSwap maker's network) and fan out
    /// to signet chain access — never testnet3, which encodes addresses
    /// identically and so mismatches without erroring.
    #[test]
    fn signet_resolves_to_signet_chain() {
        let (chain, currency) = asset_from_boltz("BTC", "signet").unwrap();

        assert_eq!(chain, Chain::Bitcoin(BitcoinChain::BitcoinSignet));
        assert_eq!(currency, Currency::Btc);

        // Liquid has no signet, so the L-BTC side pairs with Liquid testnet.
        let (chain, _) = asset_from_boltz("L-BTC", "signet").unwrap();
        assert_eq!(chain, Chain::Liquid(LiquidChain::LiquidTestnet));
    }
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
            inner: BoltzApiClientV2::new(
                base_url,
                timeout_secs.map(std::time::Duration::from_secs),
            ),
        }
    }

    /// Client pointed at the default **KaleidoSwap maker** for a network
    /// ("signet" | "regtest").
    ///
    /// "signet" is the KaleidoSwap maker and settles on Mutinynet — pair it with
    /// signet chain access, not testnet3. Rejects "testnet" (we run no testnet3
    /// maker — signet is our testing network) and "mainnet" (no mainnet maker is
    /// live yet) instead of falling back to a third party; to reach any other
    /// maker, pass an explicit base URL to the constructor.
    #[wasm_bindgen(js_name = forNetwork)]
    pub fn for_network(network: String) -> Result<BoltzClient, JsValue> {
        Ok(BoltzClient {
            inner: BoltzApiClientV2::default(parse_network(&network)?).map_err(core_err)?,
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

    // `network` ("mainnet" | "testnet" | "regtest") is used to validate the
    // returned lockup address/tree against the request before the caller funds
    // it — mirroring the checks the native bindings run.

    #[wasm_bindgen(js_name = createSubmarineSwap)]
    pub async fn create_submarine_swap(
        &self,
        network: String,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let req: CreateSubmarineRequest = from_js(req)?;
        let (from_chain, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (_, to_currency) = asset_from_boltz(&req.to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidoswap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidoswap_sdk::network::Currency::LUsdt)
        ) {
            self.inner
                .get_submarine_pairs()
                .await
                .map_err(core_err)?
                .expected_liquid_asset_context(from_currency, to_currency)
                .map_err(core_err)?
        } else {
            None
        };
        let resp = self.inner.post_swap_req(&req).await.map_err(core_err)?;
        resp.validate_with_currency_and_asset_context(
            &req.invoice,
            &req.refund_public_key,
            from_chain,
            Some(from_currency),
            expected_asset_context,
        )
        .map_err(core_err)?;
        to_js(&resp)
    }
    #[wasm_bindgen(js_name = createReverseSwap)]
    pub async fn create_reverse_swap(
        &self,
        network: String,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let req: CreateReverseRequest = from_js(req)?;
        let claim_pk = req.claim_public_key;
        let to = req.to.clone();
        let preimage_hash = req.preimage_hash;
        let invoice = req.invoice.clone();
        let (_, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (to_chain, to_currency) = asset_from_boltz(&to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidoswap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidoswap_sdk::network::Currency::LUsdt)
        ) {
            self.inner
                .get_reverse_pairs()
                .await
                .map_err(core_err)?
                .expected_liquid_asset_context(from_currency, to_currency)
                .map_err(core_err)?
        } else {
            None
        };
        let resp = self.inner.post_reverse_req(req).await.map_err(core_err)?;
        // Validate the returned tree/address regardless of request form: derive
        // the payment hash from `preimage_hash` or, in the invoice form, from the
        // invoice itself. Never hand back an unvalidated response to fund.
        let preimage = if let Some(hash) = preimage_hash {
            kaleidoswap_sdk::util::secrets::Preimage::from_sha256_str(&hash.to_string())
                .map_err(core_err)?
        } else if let Some(inv) = &invoice {
            kaleidoswap_sdk::util::secrets::Preimage::from_invoice_str(inv).map_err(core_err)?
        } else {
            return Err(JsValue::from_str(
                "reverse swap request needs preimageHash or invoice",
            ));
        };
        resp.validate_with_currency_and_asset_context(
            &preimage,
            &claim_pk,
            to_chain,
            Some(to_currency),
            expected_asset_context,
        )
        .map_err(core_err)?;
        to_js(&resp)
    }
    #[wasm_bindgen(js_name = createChainSwap)]
    pub async fn create_chain_swap(
        &self,
        network: String,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let req: CreateChainRequest = from_js(req)?;
        // Both keys are required so the returned lockup script/tree can be
        // checked against the request — never hand back an unvalidated
        // response to fund (same guarantee as the reverse path). Reject (and
        // resolve the chains) *before* posting, so a bad request can't create
        // an orphan swap server-side.
        let (claim_pk, refund_pk) = match (req.claim_public_key, req.refund_public_key) {
            (Some(claim_pk), Some(refund_pk)) => (claim_pk, refund_pk),
            _ => {
                return Err(JsValue::from_str(
                    "chain swap request needs claimPublicKey and refundPublicKey",
                ))
            }
        };
        let (from_chain, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (to_chain, to_currency) = asset_from_boltz(&req.to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidoswap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidoswap_sdk::network::Currency::LUsdt)
        ) {
            self.inner
                .get_chain_pairs()
                .await
                .map_err(core_err)?
                .expected_liquid_asset_context(from_currency, to_currency)
                .map_err(core_err)?
        } else {
            None
        };
        let (from_asset_context, to_asset_context) = match (from_currency, to_currency) {
            (kaleidoswap_sdk::network::Currency::LUsdt, _) => (expected_asset_context, None),
            (_, kaleidoswap_sdk::network::Currency::LUsdt) => (None, expected_asset_context),
            _ => (None, None),
        };
        let resp = self.inner.post_chain_req(req).await.map_err(core_err)?;
        resp.validate_with_currency_and_asset_context(
            &claim_pk,
            &refund_pk,
            from_chain,
            to_chain,
            Some(from_currency),
            Some(to_currency),
            from_asset_context,
            to_asset_context,
        )
        .map_err(core_err)?;
        to_js(&resp)
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
        to_js(
            &self
                .inner
                .get_submarine_preimage(&id)
                .await
                .map_err(core_err)?,
        )
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
        self.inner
            .accept_quote(&swap_id, amount_sat)
            .await
            .map_err(core_err)
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
        to_js(
            &self
                .inner
                .post_swap_restore(&xpub, derivation_path, gap_limit)
                .await
                .map_err(core_err)?,
        )
    }
    #[wasm_bindgen(js_name = swapRestoreIndex)]
    pub async fn swap_restore_index(
        &self,
        xpub: String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .inner
                .post_swap_restore_index(&xpub, derivation_path, gap_limit)
                .await
                .map_err(core_err)?,
        )
    }
}

// ============================================================================
// Swap-script + claim/refund transaction construction (client-side crypto).
//
// wasm-bindgen async methods can't take `&ExportedType` args (the Future would
// outlive the JS-side borrow), so construct/broadcast take primitives + a params
// object and rebuild the chain/boltz clients internally. Per-swap keys come from
// `WasmSwapMasterKey.deriveSwapKey` (returns { publicKey, secretKey } hex).
// ============================================================================

use kaleidoswap_sdk::bitcoin::hex::DisplayHex as _;
use kaleidoswap_sdk::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
use kaleidoswap_sdk::bitcoin::PublicKey;
use kaleidoswap_sdk::boltz::{
    ChainSwapDetails, CreateReverseResponse, CreateSubmarineResponse, Side,
};
use kaleidoswap_sdk::fees::Fee;
use kaleidoswap_sdk::network::esplora::{EsploraBitcoinClient, EsploraLiquidClient};
use kaleidoswap_sdk::network::Chain;
use kaleidoswap_sdk::swaps::liquid::{
    FundedLiquidPset, PreparedLiquidSpend as CorePreparedLiquidSpend,
};
use kaleidoswap_sdk::swaps::{
    BtcLikeTransaction as CoreBtcLikeTransaction, ChainClient as CoreChainClient,
    LiquidPsetParams as CoreLiquidPsetParams, SwapScript as CoreSwapScript, SwapTransactionParams,
    TransactionOptions,
};
use kaleidoswap_sdk::util::secrets::Preimage as CorePreimage;
use std::str::FromStr as _;

fn build_chain(kind: &str, network: &str) -> Result<Chain, JsValue> {
    let net = parse_network(network)?;
    match kind.to_lowercase().as_str() {
        "bitcoin" | "btc" => Ok(Chain::Bitcoin(net.into())),
        "liquid" | "lbtc" | "l-btc" => Ok(Chain::Liquid(net.into())),
        other => Err(JsValue::from_str(&format!("unknown chain kind: {other}"))),
    }
}

fn build_fee(sat_per_vb: Option<f64>, absolute_sat: Option<u64>) -> Result<Fee, JsValue> {
    match (sat_per_vb, absolute_sat) {
        (Some(r), None) => Ok(Fee::Relative(r)),
        (None, Some(a)) => Ok(Fee::Absolute(a)),
        _ => Err(JsValue::from_str(
            "provide exactly one of feeSatPerVb or feeAbsoluteSat",
        )),
    }
}

fn build_chain_client(
    network: &str,
    bitcoin_esplora_url: &Option<String>,
    liquid_esplora_url: &Option<String>,
    timeout_secs: Option<u64>,
) -> Result<CoreChainClient, JsValue> {
    let net = parse_network(network)?;
    let timeout = timeout_secs.unwrap_or(30);
    let mut cc = CoreChainClient::new();
    if let Some(url) = bitcoin_esplora_url {
        cc = cc.with_bitcoin(EsploraBitcoinClient::new(net.into(), url, timeout));
    }
    if let Some(url) = liquid_esplora_url {
        cc = cc.with_liquid(EsploraLiquidClient::new(net.into(), url, timeout));
    }
    Ok(cc)
}

fn default_true() -> bool {
    true
}

/// Parameters for building a claim/refund transaction (a plain JS object).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TxParams {
    output_address: String,
    swap_id: String,
    /// Per-swap key secret (hex), e.g. from `deriveSwapKey(index).secretKey`.
    keys_secret_hex: String,
    boltz_base_url: String,
    #[serde(default)]
    boltz_timeout_secs: Option<u64>,
    network: String,
    #[serde(default)]
    bitcoin_esplora_url: Option<String>,
    #[serde(default)]
    liquid_esplora_url: Option<String>,
    #[serde(default)]
    esplora_timeout_secs: Option<u64>,
    #[serde(default)]
    fee_sat_per_vb: Option<f64>,
    #[serde(default)]
    fee_absolute_sat: Option<u64>,
    #[serde(default = "default_true")]
    cooperative: bool,
}

impl TxParams {
    fn keypair(&self) -> Result<Keypair, JsValue> {
        let sk = SecretKey::from_str(&self.keys_secret_hex).map_err(js_err)?;
        Ok(Keypair::from_secret_key(&Secp256k1::new(), &sk))
    }
    fn chain_client(&self) -> Result<CoreChainClient, JsValue> {
        build_chain_client(
            &self.network,
            &self.bitcoin_esplora_url,
            &self.liquid_esplora_url,
            self.esplora_timeout_secs,
        )
    }
    fn boltz(&self) -> kaleidoswap_sdk::boltz::BoltzApiClientV2 {
        kaleidoswap_sdk::boltz::BoltzApiClientV2::new(
            self.boltz_base_url.clone(),
            self.boltz_timeout_secs.map(std::time::Duration::from_secs),
        )
    }
}

/// Parameters for preparing a caller-funded Liquid PSET (a plain JS object).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiquidPsetParams {
    output_address: String,
    swap_id: String,
    max_fee: u64,
    quoted_fee_cap: u64,
    boltz_base_url: String,
    #[serde(default)]
    boltz_timeout_secs: Option<u64>,
    network: String,
    liquid_esplora_url: String,
    #[serde(default)]
    esplora_timeout_secs: Option<u64>,
    /// Optional serialized Liquid lockup transaction.
    #[serde(default)]
    lockup_tx_hex: Option<String>,
}

impl LiquidPsetParams {
    fn chain_client(&self) -> Result<CoreChainClient, JsValue> {
        build_chain_client(
            &self.network,
            &None,
            &Some(self.liquid_esplora_url.clone()),
            self.esplora_timeout_secs,
        )
    }

    fn boltz(&self) -> kaleidoswap_sdk::boltz::BoltzApiClientV2 {
        kaleidoswap_sdk::boltz::BoltzApiClientV2::new(
            self.boltz_base_url.clone(),
            self.boltz_timeout_secs.map(std::time::Duration::from_secs),
        )
    }
}

/// A reconstructed swap script; builds the claim/refund transactions.
#[wasm_bindgen]
pub struct SwapScript {
    inner: CoreSwapScript,
}

#[wasm_bindgen]
impl SwapScript {
    /// Reconstruct from a submarine-swap create response.
    /// `chainKind` is "bitcoin" | "liquid"; `ourPubkeyHex` is the refund pubkey.
    #[wasm_bindgen(js_name = fromSubmarine)]
    pub fn from_submarine(
        chain_kind: String,
        network: String,
        response: JsValue,
        our_pubkey_hex: String,
    ) -> Result<SwapScript, JsValue> {
        let chain = build_chain(&chain_kind, &network)?;
        let resp: CreateSubmarineResponse = from_js(response)?;
        let pk = PublicKey::from_str(&our_pubkey_hex).map_err(js_err)?;
        Ok(SwapScript {
            inner: CoreSwapScript::submarine_from_swap_resp(chain, &resp, pk).map_err(core_err)?,
        })
    }

    /// Reconstruct from a reverse-swap create response (`ourPubkeyHex` = claim pubkey).
    #[wasm_bindgen(js_name = fromReverse)]
    pub fn from_reverse(
        chain_kind: String,
        network: String,
        response: JsValue,
        our_pubkey_hex: String,
    ) -> Result<SwapScript, JsValue> {
        let chain = build_chain(&chain_kind, &network)?;
        let resp: CreateReverseResponse = from_js(response)?;
        let pk = PublicKey::from_str(&our_pubkey_hex).map_err(js_err)?;
        Ok(SwapScript {
            inner: CoreSwapScript::reverse_from_swap_resp(chain, &resp, pk).map_err(core_err)?,
        })
    }

    /// Reconstruct from chain-swap details. `side` is "lockup" | "claim".
    #[wasm_bindgen(js_name = fromChain)]
    pub fn from_chain(
        chain_kind: String,
        network: String,
        side: String,
        chain_swap_details: JsValue,
        our_pubkey_hex: String,
    ) -> Result<SwapScript, JsValue> {
        let chain = build_chain(&chain_kind, &network)?;
        let side = match side.to_lowercase().as_str() {
            "lockup" => Side::Lockup,
            "claim" => Side::Claim,
            other => return Err(JsValue::from_str(&format!("unknown side: {other}"))),
        };
        let details: ChainSwapDetails = from_js(chain_swap_details)?;
        let pk = PublicKey::from_str(&our_pubkey_hex).map_err(js_err)?;
        Ok(SwapScript {
            inner: CoreSwapScript::chain_from_swap_resp(chain, side, details, pk)
                .map_err(core_err)?,
        })
    }

    /// Build the claim transaction. `preimageHex` is the swap preimage
    /// (e.g. `derivePreimage(index).preimage`); `params` is a `TxParams` object.
    ///
    /// Note: for **chain-swap** claims set `params.cooperative = false`. The
    /// cooperative path needs the counterparty lockup script + refund keys, which
    /// this params object does not yet carry (submarine/reverse cooperative
    /// claims work with the default `cooperative = true`).
    #[wasm_bindgen(js_name = constructClaim)]
    pub async fn construct_claim(
        &self,
        preimage_hex: String,
        params: JsValue,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let p: TxParams = from_js(params)?;
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let preimage = CorePreimage::from_str(&preimage_hex).map_err(js_err)?;
        let tx_params = SwapTransactionParams {
            keys: p.keypair()?,
            output_address: p.output_address.clone(),
            fee: build_fee(p.fee_sat_per_vb, p.fee_absolute_sat)?,
            swap_id: p.swap_id.clone(),
            chain_client: &chain_client,
            boltz_api: &boltz,
            options: Some(TransactionOptions::default().with_cooperative(p.cooperative)),
        };
        let tx = self
            .inner
            .construct_claim(&preimage, tx_params)
            .await
            .map_err(core_err)?;
        Ok(BtcLikeTransaction { inner: tx })
    }

    /// Build the refund transaction (after the timelock, or cooperatively).
    #[wasm_bindgen(js_name = constructRefund)]
    pub async fn construct_refund(&self, params: JsValue) -> Result<BtcLikeTransaction, JsValue> {
        let p: TxParams = from_js(params)?;
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let tx_params = SwapTransactionParams {
            keys: p.keypair()?,
            output_address: p.output_address.clone(),
            fee: build_fee(p.fee_sat_per_vb, p.fee_absolute_sat)?,
            swap_id: p.swap_id.clone(),
            chain_client: &chain_client,
            boltz_api: &boltz,
            options: Some(TransactionOptions::default().with_cooperative(p.cooperative)),
        };
        let tx = self
            .inner
            .construct_refund(tx_params)
            .await
            .map_err(core_err)?;
        Ok(BtcLikeTransaction { inner: tx })
    }

    /// Prepare an L-USDT claim PSET. The returned object pins the swap intent
    /// and must be retained until `finalizeClaim` is called.
    #[wasm_bindgen(js_name = prepareLiquidClaim)]
    pub async fn prepare_liquid_claim(
        &self,
        params: JsValue,
    ) -> Result<PreparedLiquidSpend, JsValue> {
        let p: LiquidPsetParams = from_js(params)?;
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let options = p
            .lockup_tx_hex
            .as_deref()
            .map(CoreBtcLikeTransaction::from_hex_liquid)
            .transpose()
            .map_err(core_err)?
            .map(|tx| TransactionOptions::default().with_lockup_tx(tx));
        let prepared = self
            .inner
            .prepare_liquid_claim(CoreLiquidPsetParams {
                output_address: p.output_address,
                max_fee: p.max_fee,
                quoted_fee_cap: p.quoted_fee_cap,
                swap_id: p.swap_id,
                chain_client: &chain_client,
                boltz_api: &boltz,
                options,
            })
            .await
            .map_err(core_err)?;
        Ok(PreparedLiquidSpend { inner: prepared })
    }

    /// Prepare an L-USDT refund PSET.
    #[wasm_bindgen(js_name = prepareLiquidRefund)]
    pub async fn prepare_liquid_refund(
        &self,
        params: JsValue,
    ) -> Result<PreparedLiquidSpend, JsValue> {
        let p: LiquidPsetParams = from_js(params)?;
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let options = p
            .lockup_tx_hex
            .as_deref()
            .map(CoreBtcLikeTransaction::from_hex_liquid)
            .transpose()
            .map_err(core_err)?
            .map(|tx| TransactionOptions::default().with_lockup_tx(tx));
        let prepared = self
            .inner
            .prepare_liquid_refund(CoreLiquidPsetParams {
                output_address: p.output_address,
                max_fee: p.max_fee,
                quoted_fee_cap: p.quoted_fee_cap,
                swap_id: p.swap_id,
                chain_client: &chain_client,
                boltz_api: &boltz,
                options,
            })
            .await
            .map_err(core_err)?;
        Ok(PreparedLiquidSpend { inner: prepared })
    }
}

/// Immutable L-USDT spend intent returned by `prepareLiquidClaim` or
/// `prepareLiquidRefund`.
#[wasm_bindgen]
pub struct PreparedLiquidSpend {
    inner: CorePreparedLiquidSpend,
}

#[wasm_bindgen]
impl PreparedLiquidSpend {
    /// Return the base64 PSET template and its pinned asset/amount/fee metadata.
    pub fn template(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.template())
    }

    /// Validate the funded PSET and add only the swap input's claim witness.
    #[wasm_bindgen(js_name = finalizeClaim)]
    pub fn finalize_claim(
        &self,
        funded_pset: JsValue,
        keys_secret_hex: String,
        preimage_hex: String,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let funded: FundedLiquidPset = from_js(funded_pset)?;
        let secret = SecretKey::from_str(&keys_secret_hex).map_err(js_err)?;
        let keys = Keypair::from_secret_key(&Secp256k1::new(), &secret);
        let preimage = CorePreimage::from_str(&preimage_hex).map_err(js_err)?;
        let tx = self
            .inner
            .finalize_claim(funded, &keys, &preimage)
            .map_err(core_err)?;
        Ok(BtcLikeTransaction {
            inner: CoreBtcLikeTransaction::liquid(tx),
        })
    }

    /// Validate the funded PSET and add only the swap input's refund witness.
    #[wasm_bindgen(js_name = finalizeRefund)]
    pub fn finalize_refund(
        &self,
        funded_pset: JsValue,
        keys_secret_hex: String,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let funded: FundedLiquidPset = from_js(funded_pset)?;
        let secret = SecretKey::from_str(&keys_secret_hex).map_err(js_err)?;
        let keys = Keypair::from_secret_key(&Secp256k1::new(), &secret);
        let tx = self
            .inner
            .finalize_refund(funded, &keys)
            .map_err(core_err)?;
        Ok(BtcLikeTransaction {
            inner: CoreBtcLikeTransaction::liquid(tx),
        })
    }
}

/// A signed Bitcoin/Liquid transaction produced by claim/refund construction.
#[wasm_bindgen]
pub struct BtcLikeTransaction {
    inner: CoreBtcLikeTransaction,
}

#[wasm_bindgen]
impl BtcLikeTransaction {
    /// The transaction serialized as hex.
    pub fn hex(&self) -> String {
        match &self.inner {
            CoreBtcLikeTransaction::Bitcoin(tx) => {
                kaleidoswap_sdk::bitcoin::consensus::serialize(tx).to_lower_hex_string()
            }
            CoreBtcLikeTransaction::Liquid(tx) => {
                kaleidoswap_sdk::elements::encode::serialize(tx).to_lower_hex_string()
            }
        }
    }

    /// The transaction id.
    pub fn txid(&self) -> String {
        match &self.inner {
            CoreBtcLikeTransaction::Bitcoin(tx) => tx.compute_txid().to_string(),
            CoreBtcLikeTransaction::Liquid(tx) => tx.txid().to_string(),
        }
    }

    /// Broadcast via an Esplora backend, returning the txid.
    pub async fn broadcast(
        &self,
        network: String,
        bitcoin_esplora_url: Option<String>,
        liquid_esplora_url: Option<String>,
        esplora_timeout_secs: Option<u64>,
    ) -> Result<String, JsValue> {
        let cc = build_chain_client(
            &network,
            &bitcoin_esplora_url,
            &liquid_esplora_url,
            esplora_timeout_secs,
        )?;
        cc.broadcast_tx(&self.inner).await.map_err(core_err)
    }
}

// ============================================================================
// WebSocket swap-status stream.
//
// JS usage:
//   const ws = new BoltzWsApi(wsUrl);
//   ws.runWsLoop();                       // do NOT await — runs in background
//   const updates = ws.updates();
//   await ws.subscribeSwap(swapId);
//   for (;;) { const status = await updates.next(); ... }
//
// `runWsLoop` is a *sync* method returning a Promise: it clones the inner Arc and
// hands it to future_to_promise, so it does not hold a `&self` borrow across the
// (never-resolving) loop — otherwise wasm-bindgen would reject any other call on
// the same object while the loop is pending.
// ============================================================================

use kaleidoswap_sdk::boltz::{BoltzWsApi as CoreBoltzWsApi, BoltzWsConfig, SwapStatus};
use tokio::sync::{broadcast, Mutex as TokioMutex};

/// Boltz WebSocket status stream.
#[wasm_bindgen]
pub struct BoltzWsApi {
    inner: Arc<CoreBoltzWsApi>,
}

#[wasm_bindgen]
impl BoltzWsApi {
    /// `new BoltzWsApi(wsUrl)` — e.g. `wss://maker.signet.kaleidoswap.com/v2/ws`
    /// (or `wss://api.boltz.exchange/v2/ws` for Boltz).
    #[wasm_bindgen(constructor)]
    pub fn new(ws_url: String) -> BoltzWsApi {
        BoltzWsApi {
            inner: Arc::new(CoreBoltzWsApi::new(ws_url, BoltzWsConfig::default())),
        }
    }

    /// Start the reconnecting WS loop in the background. Returns a Promise that
    /// resolves only on shutdown — do NOT await it in normal use.
    #[wasm_bindgen(js_name = runWsLoop)]
    pub fn run_ws_loop(&self) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            inner.run_ws_loop().await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Subscribe to status updates for a swap id.
    #[wasm_bindgen(js_name = subscribeSwap)]
    pub async fn subscribe_swap(&self, swap_id: String) -> Result<(), JsValue> {
        self.inner.subscribe_swap(&swap_id).await.map_err(core_err)
    }

    /// A cursor over swap-status updates (see `BoltzWsUpdates.next`).
    pub fn updates(&self) -> BoltzWsUpdates {
        BoltzWsUpdates {
            inner: TokioMutex::new(self.inner.updates()),
        }
    }
}

/// Cursor over the swap-status broadcast; await `next()` repeatedly.
#[wasm_bindgen]
pub struct BoltzWsUpdates {
    inner: TokioMutex<broadcast::Receiver<SwapStatus>>,
}

#[wasm_bindgen]
impl BoltzWsUpdates {
    /// Resolve with the next `SwapStatus`, or reject if the stream lagged/closed.
    pub async fn next(&self) -> Result<JsValue, JsValue> {
        let mut rx = self.inner.lock().await;
        let status = rx.recv().await.map_err(js_err)?;
        to_js(&status)
    }
}
