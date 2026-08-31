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

// ---- Errors -----------------------------------------------------------------
//
// Every rejection produced after an argument reaches Rust is a JS `Error`
// carrying a stable `code`, never a bare string. Values rejected earlier by
// wasm-bindgen's generated ABI glue (for example a Number supplied for a `u64` /
// JS `bigint`) remain native JavaScript errors and do not carry this code.

/// Code for input these bindings reject before, or instead of, reaching the core
/// SDK: a mistyped argument, an unparseable hex/enum value, or a request object
/// serde could not deserialize. Core failures keep their own code ([`core_err`]).
const INVALID_ARGUMENT: &str = "InvalidArgument";

/// Code for a failure on our side of the boundary rather than the caller's.
const INTERNAL: &str = "Internal";

/// Tag an `Error` with the `code` callers branch on, mirroring it into `name` so
/// the code is visible in a stringified error too.
fn set_code(error: &js_sys::Error, code: &str) {
    error.set_name(code);
    let _ = js_sys::Reflect::set(
        error.as_ref(),
        &JsValue::from_str("code"),
        &JsValue::from_str(code),
    );
}

fn coded_err(code: &str, message: &str) -> JsValue {
    let error = js_sys::Error::new(message);
    set_code(&error, code);
    error.into()
}

/// The caller's input is unusable. The message names the offending argument or
/// field, so the rejection is actionable without reading SDK source.
fn arg_err<E: std::fmt::Display>(e: E) -> JsValue {
    coded_err(INVALID_ARGUMENT, &e.to_string())
}

/// Something failed on our side of the boundary — not the caller's input.
fn internal_err<E: std::fmt::Display>(e: E) -> JsValue {
    coded_err(INTERNAL, &e.to_string())
}

fn to_js<T: serde::Serialize>(v: &T) -> Result<JsValue, JsValue> {
    // Serialize 64-bit integers as JS BigInt so u64 amounts cross the boundary
    // losslessly instead of being rounded through an f64. This matches
    // wasm-bindgen's own u64 <-> bigint mapping in direct signatures;
    // deserialization (from_js) accepts both Number and BigInt, so request
    // objects may use either.
    //
    // Serialize Rust maps as plain JS objects rather than `Map`. Without this,
    // the JS shape of a response follows the Rust type that produced it —
    // structs become objects, `HashMap`s become `Map`s — so one response changes
    // access style partway down. The pairs and nodes responses are a struct
    // wrapping a `HashMap<String, _>`, which made `pairs.BTC` a property read
    // while `pairs.BTC["L-BTC"]` was silently `undefined` and needed `.get()`.
    // Every map crossing this boundary is keyed by `String`, so no key is lost in
    // the conversion, and one uniform object shape is what this crate's docs and
    // the hand-written TS types on top of it already declare.
    let ser = serde_wasm_bindgen::Serializer::new()
        .serialize_large_number_types_as_bigints(true)
        .serialize_maps_as_objects(true);
    v.serialize(&ser).map_err(internal_err)
}

fn from_js<T: serde::de::DeserializeOwned>(v: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| {
        // serde-wasm-bindgen's error already *is* a JS `Error` naming the missing
        // or mistyped field. Tag that object in place; converting it through
        // `Display` instead would fold its own "Error: " prefix into the message.
        let value: JsValue = e.into();
        if let Some(error) = value.dyn_ref::<js_sys::Error>() {
            set_code(error, INVALID_ARGUMENT);
            return value;
        }
        arg_err(format!("{value:?}"))
    })
}

// ---- String arguments -------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    /// A `string` argument taken as an unconverted JS value.
    ///
    /// wasm-bindgen marshals a `String` parameter in its generated JS glue,
    /// *before* any Rust code runs: `passStringToWasm0` reads `arg.length` and
    /// `arg.charCodeAt` and hands the result to the wasm allocator. Given a
    /// non-string — the usual cause being arguments passed in the wrong order —
    /// that computes a bogus length and traps inside the allocator with
    /// `RuntimeError: memory access out of bounds`, which tells the caller
    /// nothing and cannot be caught as an `Error`.
    ///
    /// Taking the value as an extern type instead passes it through untouched
    /// (`addHeapObject`), so [`str_arg`] can reject it by name. `typescript_type`
    /// keeps the generated `.d.ts` signature `string`, and the type itself is not
    /// emitted into the TS surface.
    #[wasm_bindgen(typescript_type = "string")]
    pub type StringArg;
}

/// Convert a required string argument, naming it if it is not a string.
fn str_arg(v: StringArg, param: &str) -> Result<String, JsValue> {
    JsValue::from(v)
        .as_string()
        .ok_or_else(|| arg_err(format!("argument `{param}` must be a string")))
}

/// Convert an optional string argument. `null`/`undefined` stay `None`.
fn opt_str_arg(v: Option<StringArg>, param: &str) -> Result<Option<String>, JsValue> {
    v.map(|v| str_arg(v, param)).transpose()
}

// ============================================================================
// Swap key management (client-side crypto). This is the browser entry point for
// deriving per-swap keys and preimages; swap-script/tx construction is exposed
// incrementally on top of these.
// ============================================================================

use kaleidorg_swap_sdk::network::Network;
use kaleidorg_swap_sdk::util::secrets::{Preimage, SwapMasterKey};

/// A derived swap key, returned to JS as `{ publicKey, secretKey }` (hex).
#[derive(serde::Serialize)]
struct DerivedKey {
    #[serde(rename = "publicKey")]
    public_key: String,
    #[serde(rename = "secretKey")]
    secret_key: String,
}

/// A derived preimage and its hashes, returned to JS as
/// `{ preimage, sha256, hash160 }` (hex). A named struct rather than an ad-hoc
/// `serde_json::json!` value: `json!`'s object variant is a map, so it reached JS
/// as a `Map` while its neighbour [`DerivedKey`] arrived as a plain object, and
/// `preimage.sha256` read `undefined` against the `DerivedPreimage` interface the
/// TS SDK declares. Nothing about a preimage has dynamic keys.
#[derive(serde::Serialize)]
struct DerivedPreimage {
    preimage: String,
    sha256: String,
    hash160: String,
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
        wallet_mnemonic: StringArg,
        passphrase: Option<StringArg>,
        network: StringArg,
    ) -> Result<WasmSwapMasterKey, JsValue> {
        let wallet_mnemonic = str_arg(wallet_mnemonic, "walletMnemonic")?;
        let passphrase = opt_str_arg(passphrase, "passphrase")?;
        let network = str_arg(network, "network")?;
        let inner = SwapMasterKey::new(
            &wallet_mnemonic,
            passphrase.as_deref(),
            parse_network(&network)?,
        )
        .map_err(core_err)?;
        Ok(WasmSwapMasterKey { inner })
    }

    /// Reconstruct from the swap (rescue) mnemonic directly.
    #[wasm_bindgen(js_name = fromSwapMnemonic)]
    pub fn from_swap_mnemonic(
        mnemonic: StringArg,
        passphrase: Option<StringArg>,
        network: StringArg,
    ) -> Result<WasmSwapMasterKey, JsValue> {
        let mnemonic = str_arg(mnemonic, "mnemonic")?;
        let passphrase = opt_str_arg(passphrase, "passphrase")?;
        let network = str_arg(network, "network")?;
        let inner = SwapMasterKey::from_mnemonic(
            &mnemonic,
            passphrase.as_deref(),
            parse_network(&network)?,
        )
        .map_err(core_err)?;
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
        let kp = self.inner.derive_swapkey(index).map_err(core_err)?;
        to_js(&DerivedKey {
            public_key: kp.public_key().to_string(),
            secret_key: kp.secret_key().display_secret().to_string(),
        })
    }

    /// Derive the deterministic preimage for the swap at `index`
    /// (`sha256(privateKey)`), returned as `{ preimage, sha256, hash160 }` hex.
    #[wasm_bindgen(js_name = derivePreimage)]
    pub fn derive_preimage(&self, index: u64) -> Result<JsValue, JsValue> {
        let kp = self.inner.derive_swapkey(index).map_err(core_err)?;
        let p = Preimage::from_swap_key(&kp);
        // `Preimage::bytes` is optional in general — a preimage rebuilt from its
        // hash alone has none — but `from_swap_key` always fills it, so this
        // cannot be hit from here. Report it rather than unwrapping: the `json!`
        // value this replaced would have quietly emitted `preimage: null` against
        // a declared `string`, and an error is the honest form of "no preimage".
        let preimage = p
            .to_string()
            .ok_or_else(|| internal_err("derived preimage has no bytes"))?;
        to_js(&DerivedPreimage {
            preimage,
            sha256: p.sha256.to_string(),
            hash160: p.hash160.to_string(),
        })
    }
}

fn parse_network(s: &str) -> Result<Network, JsValue> {
    match s.to_lowercase().as_str() {
        "mainnet" => Ok(Network::Mainnet),
        "testnet" => Ok(Network::Testnet),
        "signet" => Ok(Network::Signet),
        "regtest" => Ok(Network::Regtest),
        other => Err(arg_err(format!("unknown network: {other}"))),
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

use kaleidorg_swap_sdk::boltz::{
    BoltzApiClientV2, CreateChainRequest, CreateReverseRequest, CreateSubmarineRequest,
};
use kaleidorg_swap_sdk::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};

fn core_err(e: kaleidorg_swap_sdk::error::Error) -> JsValue {
    let error = js_sys::Error::new(&e.message());
    set_code(&error, &e.name());
    error.into()
}

/// Resolve a Boltz asset identifier to its chain and currency before posting a
/// create request, so an unsupported asset cannot leave an orphan server swap.
fn asset_from_boltz(
    s: &str,
    network: &str,
) -> Result<
    (
        kaleidorg_swap_sdk::network::Chain,
        kaleidorg_swap_sdk::network::Currency,
    ),
    JsValue,
> {
    use kaleidorg_swap_sdk::network::{Chain, Currency};

    let net = parse_network(network)?;
    match s {
        "BTC" => Ok((Chain::Bitcoin(net.into()), Currency::Btc)),
        "L-BTC" => Ok((Chain::Liquid(net.into()), Currency::LBtc)),
        "L-USDT" => Ok((Chain::Liquid(net.into()), Currency::LUsdt)),
        other => Err(arg_err(format!("unsupported Boltz asset '{other}'"))),
    }
}

#[cfg(test)]
mod boltz_asset_tests {
    use super::*;
    use kaleidorg_swap_sdk::network::{BitcoinChain, Chain, Currency, LiquidChain};

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

/// `{ makerUrl, apiKey, timeoutSecs? }` for
/// [`BoltzClient::for_kaleido_maker`]. A named struct rather than positional
/// arguments: two adjacent strings, one of them a secret, is exactly the
/// signature callers transpose.
struct KaleidoMakerOptions {
    maker_url: String,
    api_key: String,
    timeout_secs: Option<u64>,
    allow_browser: bool,
}

/// The properties [`KaleidoMakerOptions`] reads, for the unknown-key check.
const KALEIDO_MAKER_OPTION_KEYS: [&str; 4] = ["makerUrl", "apiKey", "timeoutSecs", "allowBrowser"];

impl KaleidoMakerOptions {
    /// Read the options object by hand, without `from_js`.
    ///
    /// `from_js` reports a type mismatch by handing back serde's message, and
    /// serde renders the offending **value** — so
    /// `forKaleidoMaker(process.env.KALEIDOSWAP_API_KEY)`, the exact transposed
    /// call the named-options shape exists to catch, would throw
    /// `invalid type: string "kld_live_…"` and put the organization key
    /// verbatim into whatever caught it. `ApiKey::parse` is careful never to
    /// echo its input; that care is worth nothing if the value can be echoed
    /// before it ever reaches the parser.
    ///
    /// So every error below names a property and never its contents.
    fn from_js_options(options: JsValue) -> Result<Self, JsValue> {
        let shape = "forKaleidoMaker expects an options object \
                     `{ makerUrl, apiKey, timeoutSecs? }`";
        if !options.is_object() || js_sys::Array::is_array(&options) {
            return Err(arg_err(shape));
        }
        let options: js_sys::Object = options.unchecked_into();

        // Unknown properties are rejected rather than ignored. The near misses
        // are `apikey` and `timeout`, and silently ignoring the latter means a
        // client the caller believes is bounded runs with no timeout at all.
        // Property names are the caller's own literals, so naming them is safe.
        let unknown: Vec<String> = js_sys::Object::keys(&options)
            .iter()
            .filter_map(|key| key.as_string())
            .filter(|key| !KALEIDO_MAKER_OPTION_KEYS.contains(&key.as_str()))
            .collect();
        if !unknown.is_empty() {
            return Err(arg_err(format!(
                "unknown option{} {} — {shape}",
                if unknown.len() == 1 { "" } else { "s" },
                unknown.join(", "),
            )));
        }

        Ok(Self {
            maker_url: required_string_option(&options, "makerUrl")?,
            api_key: required_string_option(&options, "apiKey")?,
            timeout_secs: timeout_secs_option(&options)?,
            allow_browser: bool_option(&options, "allowBrowser")?,
        })
    }
}

/// Whether this looks like a document context — i.e. a browser.
///
/// One wasm artifact serves both Node and the browser, so there is no
/// build-time split to make this decision at. `document` is the cheapest
/// reliable divider: Node has none, and a bundle that ships to a page does.
fn in_browser() -> bool {
    js_sys::Reflect::has(&js_sys::global(), &JsValue::from_str("document")).unwrap_or(false)
}

/// An optional boolean property, defaulting to `false`.
fn bool_option(options: &js_sys::Object, name: &str) -> Result<bool, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(name)).map_err(internal_err_js)?;
    if value.is_undefined() || value.is_null() {
        return Ok(false);
    }
    value
        .as_bool()
        .ok_or_else(|| arg_err(format!("option `{name}` must be a boolean")))
}

/// A required string property, named but never quoted back.
fn required_string_option(options: &js_sys::Object, name: &str) -> Result<String, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(name)).map_err(internal_err_js)?;
    if value.is_undefined() || value.is_null() {
        return Err(arg_err(format!("option `{name}` is required")));
    }
    value
        .as_string()
        .ok_or_else(|| arg_err(format!("option `{name}` must be a string")))
}

/// The optional `timeoutSecs`, as a whole non-negative number of seconds.
///
/// A `bigint` is accepted alongside a `number`: the rest of this surface takes
/// 64-bit values as `bigint`, and a caller who reaches for one here should not
/// be told their timeout is not a number.
///
/// The upper bound is not decoration. `as` on a float is a *saturating* cast, so
/// without it `timeoutSecs: 1e20` would be accepted as `u64::MAX` — a timeout
/// tokio clamps to a deadline that never arrives, leaving a client the caller
/// believes is bounded running with none at all. That is the failure the
/// unknown-option check above exists to prevent, and a caller who spelled the
/// property right should not hit it. No value this accepts is silently changed:
/// every one below the bound casts exactly.
fn timeout_secs_option(options: &js_sys::Object) -> Result<Option<u64>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str("timeoutSecs"))
        .map_err(internal_err_js)?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(seconds) = value.as_f64() {
        // `u64::MAX as f64` rounds *up* to 2^64, so the comparison has to be
        // strict: every f64 strictly below it is within u64 and casts exactly.
        if seconds.is_finite()
            && seconds >= 0.0
            && seconds.fract() == 0.0
            && seconds < u64::MAX as f64
        {
            return Ok(Some(seconds as u64));
        }
    } else if let Some(seconds) = value.dyn_ref::<js_sys::BigInt>() {
        // `try_from` already refuses anything outside u64, so a `bigint` is
        // never truncated either.
        if let Ok(seconds) = u64::try_from(seconds.clone()) {
            return Ok(Some(seconds));
        }
    }
    Err(arg_err(
        "option `timeoutSecs` must be a whole number of seconds, not negative, \
         and within 64 bits",
    ))
}

/// A failure on our side of the boundary, from a `JsValue` that is already an
/// exception rather than a `Display` value.
fn internal_err_js(_: JsValue) -> JsValue {
    coded_err(INTERNAL, "reading the options object threw")
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
    pub fn new(base_url: StringArg, timeout_secs: Option<u64>) -> Result<BoltzClient, JsValue> {
        Ok(BoltzClient {
            inner: BoltzApiClientV2::new(
                str_arg(base_url, "baseUrl")?,
                timeout_secs.map(std::time::Duration::from_secs),
            ),
        })
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
    pub fn for_network(network: StringArg) -> Result<BoltzClient, JsValue> {
        let network = str_arg(network, "network")?;
        Ok(BoltzClient {
            inner: BoltzApiClientV2::default(parse_network(&network)?).map_err(core_err)?,
        })
    }

    /// `BoltzClient.forKaleidoMaker({ makerUrl, apiKey, timeoutSecs? })` — a
    /// client that attributes the swaps it creates to a partner organization.
    ///
    /// `apiKey` is the organization key from the partner panel, a `kld_test_…`
    /// or `kld_live_…` value. It answers "which partner organization created
    /// this swap?" and nothing else: it authorizes no claim, no refund, no fund
    /// movement and no panel access. The per-swap `swapAuth` credential the
    /// maker returns on create stays separate and unchanged.
    ///
    /// The key is bound to `makerUrl` and is never sent anywhere else — not to
    /// Esplora, not to a second maker. `makerUrl` must be `https` unless it is a
    /// loopback address, because a bearer credential over plain HTTP is readable
    /// by anything on the path. A value that cannot be a key is rejected here
    /// rather than reaching the maker as a `401`, which is the same answer a
    /// revoked key gets.
    ///
    /// # Do not use this in a browser
    ///
    /// The key is a permanent organization credential with no origin binding and
    /// no per-key rate limit. Bundled into browser JavaScript it is visible to
    /// every visitor, who can then attribute their own swaps to — or exhaust the
    /// limits of — an organization that is not theirs, and nothing in the bundle
    /// can prevent it. **This release supports server and native integrations
    /// only:** call this from Node, keep the key in server-side configuration,
    /// and leave the browser bundle on the unauthenticated `BoltzClient`
    /// constructor.
    ///
    /// One protection is also weaker here than on the server. `fetch` owns
    /// redirect handling and wasm-bindgen can set no policy on it, so a `3xx`
    /// away from the maker is caught after the fact instead of declined: the
    /// request fails naming the host that answered. The key itself is not
    /// disclosed by that hop — `fetch` drops `Authorization` when a redirect
    /// crosses origins — but the response is not the maker's.
    #[wasm_bindgen(js_name = forKaleidoMaker)]
    pub fn for_kaleido_maker(options: JsValue) -> Result<BoltzClient, JsValue> {
        let options = KaleidoMakerOptions::from_js_options(options)?;
        if !options.allow_browser && in_browser() {
            // §7 of the attribution design says server and native only for the
            // first release, and until now that was said in documentation
            // while the constructor happily ran in a page. Refusing makes the
            // code enforce what the docs promise; `allowBrowser: true` is
            // there for a deliberate exception, so the decision is at least
            // written down at the call site.
            return Err(arg_err(
                "refusing to build an attributed client in a browser: the \
                 organization API key is a permanent credential with no origin \
                 binding and no per-key rate limit, so a key in a page is \
                 visible to every visitor. Call this from Node with the key in \
                 server-side configuration, or pass `allowBrowser: true` if you \
                 have accepted that exposure.",
            ));
        }
        let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
            maker_url: options.maker_url,
            api_key: ApiKey::parse(&options.api_key).map_err(core_err)?,
            timeout: options.timeout_secs.map(std::time::Duration::from_secs),
        })
        .map_err(core_err)?;
        Ok(BoltzClient {
            inner: client.into_inner(),
        })
    }

    /// The environment the configured organization key is scoped to — `"test"`
    /// or `"live"` — or `undefined` for an unauthenticated client.
    ///
    /// Worth asserting at start-up: a `kld_test_…` key against a production
    /// maker is refused by the maker, and this says so before any swap is
    /// attempted.
    #[wasm_bindgen(getter, js_name = apiKeyEnvironment)]
    pub fn api_key_environment(&self) -> Option<String> {
        self.inner
            .api_key()
            .map(|key| key.environment().to_string())
    }

    /// The configured organization key's public identifier — the same one the
    /// partner panel shows. Safe to log; the secret half is not reachable from
    /// JS at all.
    #[wasm_bindgen(getter, js_name = apiKeyId)]
    pub fn api_key_id(&self) -> Option<String> {
        self.inner.api_key().map(|key| key.key_id().to_string())
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
        network: StringArg,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let network = str_arg(network, "network")?;
        let req: CreateSubmarineRequest = from_js(req)?;
        let (from_chain, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (_, to_currency) = asset_from_boltz(&req.to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidorg_swap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidorg_swap_sdk::network::Currency::LUsdt)
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
        network: StringArg,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let network = str_arg(network, "network")?;
        let req: CreateReverseRequest = from_js(req)?;
        let claim_pk = req.claim_public_key;
        let to = req.to.clone();
        let preimage_hash = req.preimage_hash;
        let invoice = req.invoice.clone();
        let (_, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (to_chain, to_currency) = asset_from_boltz(&to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidorg_swap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidorg_swap_sdk::network::Currency::LUsdt)
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
            kaleidorg_swap_sdk::util::secrets::Preimage::from_sha256_str(&hash.to_string())
                .map_err(core_err)?
        } else if let Some(inv) = &invoice {
            kaleidorg_swap_sdk::util::secrets::Preimage::from_invoice_str(inv).map_err(core_err)?
        } else {
            return Err(arg_err(
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
        network: StringArg,
        req: JsValue,
    ) -> Result<JsValue, JsValue> {
        let network = str_arg(network, "network")?;
        let req: CreateChainRequest = from_js(req)?;
        // Both keys are required so the returned lockup script/tree can be
        // checked against the request — never hand back an unvalidated
        // response to fund (same guarantee as the reverse path). Reject (and
        // resolve the chains) *before* posting, so a bad request can't create
        // an orphan swap server-side.
        let (claim_pk, refund_pk) = match (req.claim_public_key, req.refund_public_key) {
            (Some(claim_pk), Some(refund_pk)) => (claim_pk, refund_pk),
            _ => {
                return Err(arg_err(
                    "chain swap request needs claimPublicKey and refundPublicKey",
                ))
            }
        };
        let (from_chain, from_currency) = asset_from_boltz(&req.from, &network)?;
        let (to_chain, to_currency) = asset_from_boltz(&req.to, &network)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (kaleidorg_swap_sdk::network::Currency::LUsdt, _)
                | (_, kaleidorg_swap_sdk::network::Currency::LUsdt)
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
            (kaleidorg_swap_sdk::network::Currency::LUsdt, _) => (expected_asset_context, None),
            (_, kaleidorg_swap_sdk::network::Currency::LUsdt) => (None, expected_asset_context),
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
    pub async fn submarine_tx(&self, id: StringArg) -> Result<JsValue, JsValue> {
        let id = str_arg(id, "id")?;
        to_js(&self.inner.get_submarine_tx(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = reverseTx)]
    pub async fn reverse_tx(&self, id: StringArg) -> Result<JsValue, JsValue> {
        let id = str_arg(id, "id")?;
        to_js(&self.inner.get_reverse_tx(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = chainTxs)]
    pub async fn chain_txs(&self, id: StringArg) -> Result<JsValue, JsValue> {
        let id = str_arg(id, "id")?;
        to_js(&self.inner.get_chain_txs(&id).await.map_err(core_err)?)
    }
    #[wasm_bindgen(js_name = submarinePreimage)]
    pub async fn submarine_preimage(&self, id: StringArg) -> Result<JsValue, JsValue> {
        let id = str_arg(id, "id")?;
        to_js(
            &self
                .inner
                .get_submarine_preimage(&id)
                .await
                .map_err(core_err)?,
        )
    }
    #[wasm_bindgen(js_name = mrhBip21)]
    pub async fn mrh_bip21(&self, invoice: StringArg) -> Result<JsValue, JsValue> {
        let invoice = str_arg(invoice, "invoice")?;
        to_js(&self.inner.get_mrh_bip21(&invoice).await.map_err(core_err)?)
    }
    pub async fn swap(&self, swap_id: StringArg) -> Result<JsValue, JsValue> {
        let swap_id = str_arg(swap_id, "swapId")?;
        to_js(&self.inner.get_swap(&swap_id).await.map_err(core_err)?)
    }
    pub async fn quote(&self, swap_id: StringArg) -> Result<JsValue, JsValue> {
        let swap_id = str_arg(swap_id, "swapId")?;
        to_js(&self.inner.get_quote(&swap_id).await.map_err(core_err)?)
    }
    /// Accept a chain-swap re-quote at `amountSat`.
    ///
    /// `swapAuth` is the per-swap credential the KaleidoSwap maker returned as
    /// `swapAuth` on the create response. Accepting commits the maker's payout,
    /// so the maker authorizes it with that credential rather than with the
    /// swap id — which is not a secret. Omit it only for a maker that issues
    /// none (upstream Boltz); against KaleidoSwap the call is rejected with
    /// `401 invalid_swap_auth` and no other route resolves the re-quote, so the
    /// swap runs out its refund path instead.
    ///
    /// Persist `swapAuth` with the swap when you create it. Nothing re-issues
    /// it — `swapRestore` authenticates with an XPUB alone and does not return
    /// it.
    #[wasm_bindgen(js_name = acceptQuote)]
    pub async fn accept_quote(
        &self,
        swap_id: StringArg,
        amount_sat: u64,
        swap_auth: Option<StringArg>,
    ) -> Result<(), JsValue> {
        let swap_id = str_arg(swap_id, "swapId")?;
        let swap_auth = opt_str_arg(swap_auth, "swapAuth")?;
        self.inner
            .accept_quote(&swap_id, amount_sat, swap_auth.as_deref())
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
        xpub: StringArg,
        derivation_path: Option<StringArg>,
        gap_limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        let xpub = str_arg(xpub, "xpub")?;
        let derivation_path = opt_str_arg(derivation_path, "derivationPath")?;
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
        xpub: StringArg,
        derivation_path: Option<StringArg>,
        gap_limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        let xpub = str_arg(xpub, "xpub")?;
        let derivation_path = opt_str_arg(derivation_path, "derivationPath")?;
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

use kaleidorg_swap_sdk::bitcoin::hex::DisplayHex as _;
use kaleidorg_swap_sdk::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};
use kaleidorg_swap_sdk::bitcoin::PublicKey;
use kaleidorg_swap_sdk::boltz::{
    ChainSwapDetails, CreateReverseResponse, CreateSubmarineResponse, Side,
};
use kaleidorg_swap_sdk::fees::Fee;
use kaleidorg_swap_sdk::network::esplora::{EsploraBitcoinClient, EsploraLiquidClient};
use kaleidorg_swap_sdk::network::Chain;
use kaleidorg_swap_sdk::swaps::liquid::{
    FundedLiquidPset, PreparedLiquidSpend as CorePreparedLiquidSpend,
};
use kaleidorg_swap_sdk::swaps::{
    BtcLikeTransaction as CoreBtcLikeTransaction, ChainClient as CoreChainClient,
    LiquidPsetParams as CoreLiquidPsetParams, SwapScript as CoreSwapScript, SwapTransactionParams,
    TransactionOptions,
};
use kaleidorg_swap_sdk::util::secrets::Preimage as CorePreimage;
use std::str::FromStr as _;

// Name the argument these parse. The upstream messages say nothing about which
// input they came from, and some say nothing at all: a public key that is
// well-formed hex of the right length but not a point on the curve renders as the
// bare string "string error".

fn parse_pubkey_arg(hex: &str, param: &str) -> Result<PublicKey, JsValue> {
    PublicKey::from_str(hex)
        .map_err(|e| arg_err(format!("argument `{param}` is not a hex public key: {e}")))
}

fn parse_secret_key_arg(hex: &str, param: &str) -> Result<SecretKey, JsValue> {
    SecretKey::from_str(hex)
        .map_err(|e| arg_err(format!("argument `{param}` is not a hex secret key: {e}")))
}

fn parse_preimage_arg(hex: &str, param: &str) -> Result<CorePreimage, JsValue> {
    CorePreimage::from_str(hex)
        .map_err(|e| arg_err(format!("argument `{param}` is not a hex preimage: {e}")))
}

fn build_chain(kind: &str, network: &str) -> Result<Chain, JsValue> {
    let net = parse_network(network)?;
    match kind.to_lowercase().as_str() {
        "bitcoin" | "btc" => Ok(Chain::Bitcoin(net.into())),
        "liquid" | "lbtc" | "l-btc" => Ok(Chain::Liquid(net.into())),
        other => Err(arg_err(format!("unknown chain kind: {other}"))),
    }
}

fn build_fee(sat_per_vb: Option<f64>, absolute_sat: Option<u64>) -> Result<Fee, JsValue> {
    match (sat_per_vb, absolute_sat) {
        (Some(r), None) => Ok(Fee::Relative(r)),
        (None, Some(a)) => Ok(Fee::Absolute(a)),
        _ => Err(arg_err(
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
        Self::keypair_from(&self.keys_secret_hex, "keysSecretHex")
    }
    fn keypair_from(secret_hex: &str, param: &str) -> Result<Keypair, JsValue> {
        let sk = parse_secret_key_arg(secret_hex, param)?;
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
    fn boltz(&self) -> kaleidorg_swap_sdk::boltz::BoltzApiClientV2 {
        kaleidorg_swap_sdk::boltz::BoltzApiClientV2::new(
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

    fn boltz(&self) -> kaleidorg_swap_sdk::boltz::BoltzApiClientV2 {
        kaleidorg_swap_sdk::boltz::BoltzApiClientV2::new(
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
        chain_kind: StringArg,
        network: StringArg,
        response: JsValue,
        our_pubkey_hex: StringArg,
    ) -> Result<SwapScript, JsValue> {
        let chain_kind = str_arg(chain_kind, "chainKind")?;
        let network = str_arg(network, "network")?;
        let our_pubkey_hex = str_arg(our_pubkey_hex, "ourPubkeyHex")?;
        let chain = build_chain(&chain_kind, &network)?;
        let resp: CreateSubmarineResponse = from_js(response)?;
        let pk = parse_pubkey_arg(&our_pubkey_hex, "ourPubkeyHex")?;
        Ok(SwapScript {
            inner: CoreSwapScript::submarine_from_swap_resp(chain, &resp, pk).map_err(core_err)?,
        })
    }

    /// Reconstruct from a reverse-swap create response (`ourPubkeyHex` = claim pubkey).
    #[wasm_bindgen(js_name = fromReverse)]
    pub fn from_reverse(
        chain_kind: StringArg,
        network: StringArg,
        response: JsValue,
        our_pubkey_hex: StringArg,
    ) -> Result<SwapScript, JsValue> {
        let chain_kind = str_arg(chain_kind, "chainKind")?;
        let network = str_arg(network, "network")?;
        let our_pubkey_hex = str_arg(our_pubkey_hex, "ourPubkeyHex")?;
        let chain = build_chain(&chain_kind, &network)?;
        let resp: CreateReverseResponse = from_js(response)?;
        let pk = parse_pubkey_arg(&our_pubkey_hex, "ourPubkeyHex")?;
        Ok(SwapScript {
            inner: CoreSwapScript::reverse_from_swap_resp(chain, &resp, pk).map_err(core_err)?,
        })
    }

    /// Reconstruct from chain-swap details. `side` is "lockup" | "claim".
    #[wasm_bindgen(js_name = fromChain)]
    pub fn from_chain(
        chain_kind: StringArg,
        network: StringArg,
        side: StringArg,
        chain_swap_details: JsValue,
        our_pubkey_hex: StringArg,
    ) -> Result<SwapScript, JsValue> {
        let chain_kind = str_arg(chain_kind, "chainKind")?;
        let network = str_arg(network, "network")?;
        let side = str_arg(side, "side")?;
        let our_pubkey_hex = str_arg(our_pubkey_hex, "ourPubkeyHex")?;
        let chain = build_chain(&chain_kind, &network)?;
        let side = match side.to_lowercase().as_str() {
            "lockup" => Side::Lockup,
            "claim" => Side::Claim,
            other => return Err(arg_err(format!("unknown side: {other}"))),
        };
        let details: ChainSwapDetails = from_js(chain_swap_details)?;
        let pk = parse_pubkey_arg(&our_pubkey_hex, "ourPubkeyHex")?;
        Ok(SwapScript {
            inner: CoreSwapScript::chain_from_swap_resp(chain, side, details, pk)
                .map_err(core_err)?,
        })
    }

    /// Build the claim transaction. `preimageHex` is the swap preimage
    /// (e.g. `derivePreimage(index).preimage`); `params` is a `TxParams` object.
    ///
    /// Note: for **chain-swap** claims set `params.cooperative = false`. This is
    /// the script-spend path. `TxParams` cannot carry the lockup script and
    /// refund key that a cooperative chain claim signs against, so reach the
    /// cheaper MuSig2 keyspend through `constructCooperativeClaim` rather than
    /// through this method. Submarine and reverse cooperative claims need
    /// nothing extra and work with the default `cooperative = true`.
    #[wasm_bindgen(js_name = constructClaim)]
    pub async fn construct_claim(
        &self,
        preimage_hex: StringArg,
        params: JsValue,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let preimage_hex = str_arg(preimage_hex, "preimageHex")?;
        let p: TxParams = from_js(params)?;
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let preimage = parse_preimage_arg(&preimage_hex, "preimageHex")?;
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

    /// Build a **cooperative** chain-swap claim (MuSig2 keyspend).
    ///
    /// `lockupScript` is our own lockup side, i.e.
    /// `SwapScript.fromChain(chainKind, network, "lockup", lockupDetails, ourPubkey)`
    /// — the cooperative path signs a temporary refund against it to obtain the
    /// server's signature for the claim, which is why `constructClaim` alone
    /// cannot do this and documents `cooperative = false` for chain swaps.
    ///
    /// `refundKeysSecretHex` is the swap's **refund** key, the counterpart of the
    /// `refundPublicKey` the swap was created with. It is a separate argument from
    /// `params.keysSecretHex` (the claim key) because a chain swap carries two
    /// independent keys, and the temporary refund is partial-signed with this one.
    /// Passing the claim key here when the two differ yields a partial signature
    /// the server rejects.
    ///
    /// The keyspend witness is much smaller than the script path's, and
    /// `feeSatPerVb` is applied to it correctly — the fee is computed against a
    /// stubbed cooperative witness, so a rate needs no adjustment for this path.
    ///
    /// `params.cooperative` is rejected if set to `false`: this method is the
    /// cooperative path by construction, and honoring the flag is what
    /// `constructClaim` is for.
    #[wasm_bindgen(js_name = constructCooperativeClaim)]
    pub async fn construct_cooperative_claim(
        &self,
        preimage_hex: StringArg,
        params: JsValue,
        lockup_script: &SwapScript,
        refund_keys_secret_hex: StringArg,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let preimage_hex = str_arg(preimage_hex, "preimageHex")?;
        let refund_keys_secret_hex = str_arg(refund_keys_secret_hex, "refundKeysSecretHex")?;
        let p: TxParams = from_js(params)?;
        if !p.cooperative {
            return Err(arg_err(
                "constructCooperativeClaim cannot honor cooperative = false; \
                 use constructClaim for a script-path chain claim",
            ));
        }
        let chain_client = p.chain_client()?;
        let boltz = p.boltz();
        let preimage = parse_preimage_arg(&preimage_hex, "preimageHex")?;
        let keys = p.keypair()?;
        let refund_keys = TxParams::keypair_from(&refund_keys_secret_hex, "refundKeysSecretHex")?;
        let tx_params = SwapTransactionParams {
            keys,
            output_address: p.output_address.clone(),
            fee: build_fee(p.fee_sat_per_vb, p.fee_absolute_sat)?,
            swap_id: p.swap_id.clone(),
            chain_client: &chain_client,
            boltz_api: &boltz,
            options: Some(
                TransactionOptions::default()
                    .with_chain_claim(refund_keys, lockup_script.inner.clone()),
            ),
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
        keys_secret_hex: StringArg,
        preimage_hex: StringArg,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let keys_secret_hex = str_arg(keys_secret_hex, "keysSecretHex")?;
        let preimage_hex = str_arg(preimage_hex, "preimageHex")?;
        let funded: FundedLiquidPset = from_js(funded_pset)?;
        let secret = parse_secret_key_arg(&keys_secret_hex, "keysSecretHex")?;
        let keys = Keypair::from_secret_key(&Secp256k1::new(), &secret);
        let preimage = parse_preimage_arg(&preimage_hex, "preimageHex")?;
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
        keys_secret_hex: StringArg,
    ) -> Result<BtcLikeTransaction, JsValue> {
        let keys_secret_hex = str_arg(keys_secret_hex, "keysSecretHex")?;
        let funded: FundedLiquidPset = from_js(funded_pset)?;
        let secret = parse_secret_key_arg(&keys_secret_hex, "keysSecretHex")?;
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
                kaleidorg_swap_sdk::bitcoin::consensus::serialize(tx).to_lower_hex_string()
            }
            CoreBtcLikeTransaction::Liquid(tx) => {
                kaleidorg_swap_sdk::elements::encode::serialize(tx).to_lower_hex_string()
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
        network: StringArg,
        bitcoin_esplora_url: Option<StringArg>,
        liquid_esplora_url: Option<StringArg>,
        esplora_timeout_secs: Option<u64>,
    ) -> Result<String, JsValue> {
        let network = str_arg(network, "network")?;
        let bitcoin_esplora_url = opt_str_arg(bitcoin_esplora_url, "bitcoinEsploraUrl")?;
        let liquid_esplora_url = opt_str_arg(liquid_esplora_url, "liquidEsploraUrl")?;
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

use kaleidorg_swap_sdk::boltz::{BoltzWsApi as CoreBoltzWsApi, BoltzWsConfig, SwapStatus};
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
    pub fn new(ws_url: StringArg) -> Result<BoltzWsApi, JsValue> {
        Ok(BoltzWsApi {
            inner: Arc::new(CoreBoltzWsApi::new(
                str_arg(ws_url, "wsUrl")?,
                BoltzWsConfig::default(),
            )),
        })
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
    pub async fn subscribe_swap(&self, swap_id: StringArg) -> Result<(), JsValue> {
        let swap_id = str_arg(swap_id, "swapId")?;
        self.inner.subscribe_swap(&swap_id).await.map_err(core_err)
    }

    /// Whether the loop currently holds a live socket.
    ///
    /// `runWsLoop` reconnects rather than returning, which is what you want
    /// for a long watch — but it means a dropped connection is invisible from
    /// JS: updates simply stop arriving, indistinguishable from a quiet swap.
    /// This is the signal that tells the two apart.
    #[wasm_bindgen(js_name = isConnected)]
    pub async fn is_connected(&self) -> bool {
        self.inner.is_connected().await
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
        let status = rx.recv().await.map_err(internal_err)?;
        to_js(&status)
    }
}
