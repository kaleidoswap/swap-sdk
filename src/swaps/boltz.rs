//!
//! ### Boltz v2 API
//! ## Estimate fees
//!
//! ### Example
//! ```
//! let client = BoltzApiClient::new(BOLTZ_MAINNET_URL);
//! let pairs = client.get_pairs()?;
//! let btc_pair = pairs.get_btc_pair();
//! let output_amount = 75_000;
//! let base_fees = btc_pair.fees.reverse_base(output_amount)?;
//! let claim_fee = btc_pair.fees.reverse_claim_estimate();
//! println!("CALCULATED FEES: {}", base_fees);
//! println!("ONCHAIN LOCKUP: {}", output_amount - base_fees);
//! println!(
//!     "ONCHAIN RECIEVABLE: {}",
//!     output_amount - base_fees - claim_fee
//! );

use crate::kaleido::{ApiKey, API_KEY_HEADER};
use crate::network::{Currency, Network};
#[cfg(feature = "ws")]
use crate::util::ensure_rustls_crypto_provider;
use crate::{error::Error, network::Chain, util::secrets::Preimage};
use crate::{BtcSwapScript, LiquidAssetContext, LiquidSwapScript};
use bitcoin::secp256k1;
use bitcoin::{hashes::sha256, hex::DisplayHex, PublicKey};
use lightning_invoice::Bolt11Invoice;
use reqwest::header::HeaderValue;
use reqwest::Method;
use secp256k1_musig::musig;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::time::Duration;

pub const BOLTZ_TESTNET_URL_V2: &str = "https://api.testnet.boltz.exchange/v2";
pub const BOLTZ_MAINNET_URL_V2: &str = "https://api.boltz.exchange/v2";
pub const BOLTZ_REGTEST: &str = "http://localhost:9001/v2";
/// The KaleidoSwap maker — the SDK's [`Network::Signet`] default. It settles on
/// Mutinynet, so pair it with [`BitcoinChain::BitcoinSignet`] chain access
/// rather than testnet3.
///
/// [`BitcoinChain::BitcoinSignet`]: crate::network::BitcoinChain::BitcoinSignet
pub const KALEIDOSWAP_SIGNET_URL_V2: &str = "https://maker.signet.kaleidoswap.com/v2";

/// Header carrying the per-swap taker credential the KaleidoSwap maker issues
/// as `swapAuth` on a create response.
///
/// See [`CreateChainResponse::swap_auth`] for what the credential is and
/// [`BoltzApiClientV2::accept_quote`] for the one route that needs it.
pub const SWAP_AUTH_HEADER: &str = "X-Swap-Auth";

#[cfg(feature = "ws")]
pub use crate::swaps::status_stream::{BoltzWsApi, BoltzWsConfig};
use reqwest::RequestBuilder;
#[cfg(feature = "ws")]
pub use tokio_tungstenite_wasm;
#[cfg(feature = "ws")]
use tokio_tungstenite_wasm::{connect, connect_with_protocols, WebSocketStream};

#[derive(Serialize, Deserialize, Debug)]
pub struct HeightResponse {
    #[serde(rename = "BTC")]
    pub btc: u32,
    #[serde(rename = "L-BTC")]
    pub lbtc: u32,
}

fn check_limits_within(maximal: u64, minimal: u64, output_amount: u64) -> Result<(), Error> {
    if output_amount < minimal {
        return Err(Error::Protocol(format!(
            "Output amount is below minimum {minimal}"
        )));
    }
    if output_amount > maximal {
        return Err(Error::Protocol(format!(
            "Output amount is above maximum {maximal}"
        )));
    }
    Ok(())
}

/// Various limits of swap parameters
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PairLimits {
    /// Maximum swap amount
    pub maximal: u64,
    /// Minimum swap amount
    pub minimal: u64,
    /// Maximum amount allowed for zero-conf
    pub maximal_zero_conf: u64,
}

impl PairLimits {
    /// Check whether the output amount intended is within the Limits
    pub fn within(&self, output_amount: u64) -> Result<(), Error> {
        check_limits_within(self.maximal, self.minimal, output_amount)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmarinePairLimits {
    /// Maximum swap amount
    pub maximal: u64,
    /// Minimum swap amount
    pub minimal: u64,
    /// Maximum amount allowed for zero-conf
    pub maximal_zero_conf: u64,
    /// Minimum batch swap amount
    pub minimal_batched: Option<u64>,
}

impl SubmarinePairLimits {
    /// Check whether the output amount intended is within the Limits
    pub fn within(&self, output_amount: u64) -> Result<(), Error> {
        let minimal = self.minimal_batched.unwrap_or(self.minimal);
        check_limits_within(self.maximal, minimal, output_amount)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReverseLimits {
    /// Maximum swap amount
    pub maximal: u64,
    /// Minimum swap amount
    pub minimal: u64,
}

impl ReverseLimits {
    /// Check whether the output amount intended is within the Limits
    pub fn within(&self, output_amount: u64) -> Result<(), Error> {
        check_limits_within(self.maximal, self.minimal, output_amount)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PairMinerFees {
    pub lockup: u64,
    pub claim: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChainMinerFees {
    pub server: u64,
    pub user: PairMinerFees,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChainFees {
    pub percentage: f64,
    pub miner_fees: ChainMinerFees,
}

impl ChainFees {
    pub fn total(&self, amount_sat: u64) -> u64 {
        self.boltz(amount_sat) + self.claim_estimate() + self.lockup() + self.server()
    }

    pub fn boltz(&self, amount_sat: u64) -> u64 {
        ((self.percentage / 100.0) * amount_sat as f64).ceil() as u64
    }

    pub fn claim_estimate(&self) -> u64 {
        self.miner_fees.user.claim
    }

    pub fn lockup(&self) -> u64 {
        self.miner_fees.user.lockup
    }

    pub fn server(&self) -> u64 {
        self.miner_fees.server
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReverseFees {
    pub percentage: f64,
    pub miner_fees: PairMinerFees,
}

impl ReverseFees {
    pub fn total(&self, invoice_amount_sat: u64) -> u64 {
        self.boltz(invoice_amount_sat) + self.claim_estimate() + self.lockup()
    }

    pub fn boltz(&self, invoice_amount_sat: u64) -> u64 {
        ((self.percentage / 100.0) * invoice_amount_sat as f64).ceil() as u64
    }

    pub fn claim_estimate(&self) -> u64 {
        self.miner_fees.claim
    }

    pub fn lockup(&self) -> u64 {
        self.miner_fees.lockup
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmarineFees {
    /// The percentage of the "send amount" that is charged by Boltz as "Boltz Fee".
    pub percentage: f64,
    /// The network fees charged for locking up and claiming funds onchain. These values are absolute, denominated in 10 ** -8 of the quote asset.
    pub miner_fees: u64,
}

impl SubmarineFees {
    pub fn total(&self, invoice_amount_sat: u64) -> u64 {
        self.boltz(invoice_amount_sat) + self.network()
    }

    pub fn boltz(&self, invoice_amount_sat: u64) -> u64 {
        ((self.percentage / 100.0) * invoice_amount_sat as f64).ceil() as u64
    }

    pub fn network(&self) -> u64 {
        self.miner_fees
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChainPair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: PairLimits,
    /// Total fees required for the swap
    pub fees: ChainFees,
    /// Asset locked on the input side when it is a Liquid asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_asset_id: Option<String>,
    /// Asset paid on the output side when it is a Liquid asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_asset_id: Option<String>,
    /// Elements policy asset used for transaction fees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReversePair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: ReverseLimits,
    /// Total fees required for the swap
    pub fees: ReverseFees,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmarinePair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: SubmarinePairLimits,
    /// Total fees required for the swap
    pub fees: SubmarineFees,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSubmarinePairsResponse {
    // `default`: a maker need not serve every asset (e.g. the KaleidoSwap
    // maker advertises no L-BTC submarine pairs) — a missing key is an empty
    // map, not a deserialization failure.
    #[serde(rename = "BTC", default)]
    pub btc: HashMap<String, SubmarinePair>,
    #[serde(rename = "L-BTC", default)]
    pub lbtc: HashMap<String, SubmarinePair>,
    #[serde(rename = "L-USDT", default)]
    pub lusdt: HashMap<String, SubmarinePair>,
}

impl GetSubmarinePairsResponse {
    /// Get the BtcBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_btc_to_btc_pair(&self) -> Option<SubmarinePair> {
        self.btc.get("BTC").cloned()
    }

    /// Get the BtcLBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_btc_to_lbtc_pair(&self) -> Option<SubmarinePair> {
        self.btc.get("L-BTC").cloned()
    }

    /// Get the LBtcBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_lbtc_to_btc_pair(&self) -> Option<SubmarinePair> {
        self.lbtc.get("BTC").cloned()
    }

    /// Get the LBtcLBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_lbtc_to_lbtc_pair(&self) -> Option<SubmarinePair> {
        self.lbtc.get("L-BTC").cloned()
    }

    /// Get the L-USDT to BTC pair data from the response.
    pub fn get_lusdt_to_btc_pair(&self) -> Option<SubmarinePair> {
        self.lusdt.get("BTC").cloned()
    }

    /// Resolve the Liquid assets committed by the selected public pair card.
    pub fn expected_liquid_asset_context(
        &self,
        from: Currency,
        to: Currency,
    ) -> Result<Option<LiquidAssetContext>, Error> {
        match (from, to) {
            (Currency::LUsdt, Currency::Btc) => self
                .lusdt
                .get("BTC")
                .ok_or_else(|| Error::Protocol("L-USDT/BTC submarine pair missing".to_string()))
                .and_then(|pair| {
                    require_pair_asset_context(
                        pair.from_asset_id.as_deref(),
                        pair.fee_asset_id.as_deref(),
                        "L-USDT/BTC submarine pair",
                    )
                    .map(Some)
                }),
            (Currency::LUsdt, _) | (_, Currency::LUsdt) => Err(Error::Protocol(
                "Unsupported L-USDT submarine pair".to_string(),
            )),
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReversePairsResponse {
    // `default`: tolerate a maker with no BTC reverse pairs (see the
    // submarine-pairs note).
    #[serde(rename = "BTC", default)]
    pub btc: HashMap<String, ReversePair>,
}

impl GetReversePairsResponse {
    /// Get the BtcBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_btc_to_btc_pair(&self) -> Option<ReversePair> {
        self.btc.get("BTC").cloned()
    }

    /// Get the BtcLBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_btc_to_lbtc_pair(&self) -> Option<ReversePair> {
        self.btc.get("L-BTC").cloned()
    }

    /// Get the BTC to L-USDT pair data from the response.
    pub fn get_btc_to_lusdt_pair(&self) -> Option<ReversePair> {
        self.btc.get("L-USDT").cloned()
    }

    /// Resolve the Liquid assets committed by the selected public pair card.
    pub fn expected_liquid_asset_context(
        &self,
        from: Currency,
        to: Currency,
    ) -> Result<Option<LiquidAssetContext>, Error> {
        match (from, to) {
            (Currency::Btc, Currency::LUsdt) => self
                .btc
                .get("L-USDT")
                .ok_or_else(|| Error::Protocol("BTC/L-USDT reverse pair missing".to_string()))
                .and_then(|pair| {
                    require_pair_asset_context(
                        pair.to_asset_id.as_deref(),
                        pair.fee_asset_id.as_deref(),
                        "BTC/L-USDT reverse pair",
                    )
                    .map(Some)
                }),
            (Currency::LUsdt, _) | (_, Currency::LUsdt) => Err(Error::Protocol(
                "Unsupported L-USDT reverse pair".to_string(),
            )),
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChainPairsResponse {
    // `default`: see the submarine-pairs note.
    #[serde(rename = "BTC", default)]
    pub btc: HashMap<String, ChainPair>,
    #[serde(rename = "L-BTC", default)]
    pub lbtc: HashMap<String, ChainPair>,
}

impl GetChainPairsResponse {
    /// Get the BtcLBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_btc_to_lbtc_pair(&self) -> Option<ChainPair> {
        self.btc.get("L-BTC").cloned()
    }

    /// Get the LBtcBtc Pair data from the response.
    /// Returns None if not found.
    pub fn get_lbtc_to_btc_pair(&self) -> Option<ChainPair> {
        self.lbtc.get("BTC").cloned()
    }

    /// Get the BTC to L-USDT atomic pair data from the response.
    pub fn get_btc_to_lusdt_pair(&self) -> Option<ChainPair> {
        self.btc.get("L-USDT").cloned()
    }

    /// Resolve the Liquid assets committed by the selected public pair card.
    pub fn expected_liquid_asset_context(
        &self,
        from: Currency,
        to: Currency,
    ) -> Result<Option<LiquidAssetContext>, Error> {
        match (from, to) {
            (Currency::Btc, Currency::LUsdt) => self
                .btc
                .get("L-USDT")
                .ok_or_else(|| Error::Protocol("BTC/L-USDT chain pair missing".to_string()))
                .and_then(|pair| {
                    require_pair_asset_context(
                        pair.to_asset_id.as_deref(),
                        pair.fee_asset_id.as_deref(),
                        "BTC/L-USDT chain pair",
                    )
                    .map(Some)
                }),
            (Currency::LUsdt, _) | (_, Currency::LUsdt) => {
                Err(Error::Protocol("Unsupported L-USDT chain pair".to_string()))
            }
            _ => Ok(None),
        }
    }
}

fn require_pair_asset_context(
    asset_id: Option<&str>,
    fee_asset_id: Option<&str>,
    pair: &str,
) -> Result<LiquidAssetContext, Error> {
    LiquidAssetContext::from_asset_ids(asset_id, fee_asset_id)?.ok_or_else(|| {
        Error::Protocol(format!(
            "{pair} must provide both the swap and fee asset ids"
        ))
    })
}

/// Whether two URLs share an origin — scheme, host and effective port.
///
/// The unit a credential is scoped to. A different port on the same host is a
/// different server, and `http` to a host is a different server from `https` to
/// it, so neither is folded away here.
pub(crate) fn same_origin(a: &reqwest::Url, b: &reqwest::Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.port_or_known_default() == b.port_or_known_default()
}

/// Whether a *single* hop from `sent_to` to `came_from` made an HTTP client strip
/// [`API_KEY_HEADER`] on the way.
///
/// Deliberately *not* [`same_origin`]. `reqwest` compares host and effective
/// port and ignores the scheme entirely, so this mirrors that rule rather than
/// the stricter one the SDK uses to decide what counts as the maker. The gap
/// between the two is `https://h` → `http://h:443`, where the key is re-sent in
/// the clear.
///
/// "Single" is the load-bearing word, and the reason the caller of this cannot
/// promise the key stayed put. `reqwest` applies its rule to each hop against
/// the one before it (`remove_sensitive_headers(headers, next, previous)`),
/// while a [`reqwest::Response`] carries only the URL the chain *ended* at — no
/// hop list, no `redirected` flag, on either target. So `https://maker` →
/// `http://maker:443` → `https://elsewhere` ends somewhere this returns `true`
/// for, having leaked the key in the clear on the first hop.
fn redirect_strips_api_key(sent_to: &reqwest::Url, came_from: &reqwest::Url) -> bool {
    sent_to.host_str() != came_from.host_str()
        || sent_to.port_or_known_default() != came_from.port_or_known_default()
}

/// What a caller should do about the organization API key after a redirect off
/// the maker. See [`BoltzApiClientV2::reject_credential_leaking_redirect`].
///
/// Neither branch claims more than the SDK can see, which is the origin the
/// response came back from and nothing about how it got there — see
/// [`redirect_strips_api_key`]. So the reassuring branch is scoped to the hop it
/// can actually account for, and names the chain it cannot: an intermediate hop
/// that changed only the scheme would have kept the key, and a partner told
/// flatly that there was nothing to revoke would leave a leaked permanent
/// credential live.
fn api_key_redirect_advice(sent_to: &reqwest::Url, came_from: &reqwest::Url) -> String {
    if redirect_strips_api_key(sent_to, came_from) {
        return format!(
            "; the response came back from a different host or port, so a direct hop \
             there dropped {API_KEY_HEADER} and the organization API key did not \
             travel — but nothing this response says came from the maker. If the \
             maker answered with a chain of redirects rather than one, an earlier hop \
             could have changed only the scheme and kept the key: this SDK sees only \
             where the chain ended, so treat the key as exposed unless you can rule \
             that out"
        );
    }
    "; the redirect changed only the scheme, which native HTTP clients do not treat \
     as cross-origin — assume the organization API key reached that host, in the \
     clear if the hop was to http, and revoke it"
        .to_string()
}

/// Reference Documnetation: <https://api.boltz.exchange/swagger>
#[derive(Debug, Clone)]
pub struct BoltzApiClientV2 {
    base_url: String,
    http_client: reqwest::Client,
    timeout: Option<Duration>,
    /// The partner organization API key, when one was configured through
    /// [`KaleidoMakerClient`]. `None` for every generic constructor here — the
    /// Boltz-compatible client authenticates nothing.
    ///
    /// [`KaleidoMakerClient`]: crate::kaleido::KaleidoMakerClient
    api_key: Option<ApiKey>,
}

impl BoltzApiClientV2 {
    pub fn new(base_url: String, timeout: Option<Duration>) -> Self {
        Self {
            base_url,
            http_client: Self::default_http_client(),
            timeout,
            api_key: None,
        }
    }

    /// The client every constructor here builds: `reqwest::Client::new` except
    /// that it never follows a redirect.
    ///
    /// [`Self::accept_quote`] sends the per-swap credential in
    /// [`SWAP_AUTH_HEADER`], and reqwest strips only `Authorization`, `Cookie`
    /// and `Proxy-Authorization` when a redirect crosses origins — a custom
    /// header rides along to whatever host the `Location` names. Against a
    /// plain-HTTP maker ([`BOLTZ_REGTEST`], or a self-hosted one) a network
    /// attacker answering `302` would collect the taker's full capability over
    /// that swap; a compromised maker could hand it to a third party the same
    /// way. The maker API redirects nowhere, so declining to follow costs
    /// nothing: a stray `3xx` surfaces as its own status instead of being
    /// chased.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn default_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            // Same failure mode as `reqwest::Client::new`, which panics here
            // too: nothing in this builder can fail but TLS backend init.
            .expect("reqwest client with a redirect policy and no other configuration")
    }

    /// `reqwest` sets no `RequestInit.redirect` on wasm, so `fetch` follows
    /// redirects and there is no policy to set here. A cross-origin hop does
    /// need a CORS preflight for [`SWAP_AUTH_HEADER`], but the host the
    /// `Location` names answers that preflight itself, so it is no barrier.
    /// [`Self::reject_credential_leaking_redirect`] catches it after the fact
    /// instead — the browser path can report the disclosure, not prevent it.
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    fn default_http_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    /// Client pointed at the default **KaleidoSwap maker** for a network.
    ///
    /// Every value this returns is a KaleidoSwap endpoint. A network we run no
    /// maker on is an error, never a third-party fallback: the caller asked for
    /// *our* maker, and quietly handing them somebody else's would put their
    /// swap in front of a counterparty they never chose.
    ///
    /// - `Signet` → the KaleidoSwap maker ([`KALEIDOSWAP_SIGNET_URL_V2`]), which
    ///   settles on Mutinynet. Use [`BitcoinChain::BitcoinSignet`] chain access
    ///   with it; testnet3 endpoints cannot see these transactions.
    /// - `Regtest` → the local regtest harness ([`BOLTZ_REGTEST`]).
    /// - `Testnet` → **errors**: KaleidoSwap runs no testnet3 maker. Our testing
    ///   network is signet — use [`Network::Signet`].
    /// - `Mainnet` → **errors**: no mainnet KaleidoSwap maker is live yet.
    ///
    /// Third-party makers stay reachable, but only by name: pass an explicit
    /// `base_url` to [`BoltzApiClientV2::new`] (e.g. [`BOLTZ_MAINNET_URL_V2`] or
    /// [`BOLTZ_TESTNET_URL_V2`] for Boltz).
    ///
    /// [`BitcoinChain::BitcoinSignet`]: crate::network::BitcoinChain::BitcoinSignet
    pub fn default(network: Network) -> Result<Self, Error> {
        let base_url = match network {
            Network::Mainnet => {
                return Err(Error::Protocol(
                    "no mainnet KaleidoSwap maker yet — pass an explicit base_url \
                     via BoltzApiClientV2::new"
                        .to_string(),
                ))
            }
            Network::Testnet => {
                return Err(Error::Protocol(
                    "no KaleidoSwap testnet3 maker — our maker runs on signet, so use \
                     Network::Signet; for a third-party testnet3 maker pass an explicit \
                     base_url via BoltzApiClientV2::new (e.g. BOLTZ_TESTNET_URL_V2)"
                        .to_string(),
                ))
            }
            Network::Signet => KALEIDOSWAP_SIGNET_URL_V2.to_string(),
            Network::Regtest => BOLTZ_REGTEST.to_string(),
        };
        Ok(Self::new(base_url, None))
    }

    /// Client over a caller-supplied `reqwest::Client`, keeping its proxy, TLS
    /// and pool configuration.
    ///
    /// Build it with [`reqwest::redirect::Policy::none`] if it will carry a
    /// `swapAuth`: reqwest forwards custom headers across a cross-origin
    /// redirect, so a redirect-following client can hand [`SWAP_AUTH_HEADER`]
    /// to another host. [`Self::new`] does this for you — see
    /// [`Self::default_http_client`].
    pub fn with_client(
        base_url: String,
        http_client: reqwest::Client,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            base_url,
            http_client,
            timeout,
            api_key: None,
        }
    }

    /// Bind a partner organization API key to this client.
    ///
    /// Crate-private: a key may only be paired with a URL [`KaleidoMakerClient`]
    /// has already vetted, which is what makes "this client sends the key" and
    /// "this client is allowed to" the same statement.
    ///
    /// [`KaleidoMakerClient`]: crate::kaleido::KaleidoMakerClient
    pub(crate) fn with_api_key(mut self, api_key: ApiKey) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// The partner organization API key this client carries, if any. The key's
    /// secret is not reachable through it — see [`ApiKey`].
    pub fn api_key(&self) -> Option<&ApiKey> {
        self.api_key.as_ref()
    }

    /// Returns the WebSocket URL for the Boltz server.
    ///
    /// The organization API key is not carried on it. `/v2/ws` publishes swap
    /// status for ids the subscriber already holds and creates nothing, so there
    /// is nothing on it to attribute — and the WebSocket client this SDK uses
    /// sends no custom headers on the handshake in the browser regardless.
    pub fn get_ws_url(&self) -> String {
        self.base_url.clone().replace("http", "ws") + "/ws"
    }

    /// Returns the web socket connection to the boltz server
    #[cfg(feature = "ws")]
    pub async fn connect_ws(&self) -> Result<WebSocketStream, Error> {
        ensure_rustls_crypto_provider();
        Ok(connect(self.get_ws_url()).await?)
    }

    /// Same as `connect_ws` but with protocols
    #[cfg(feature = "ws")]
    pub async fn connect_ws_with_protocols(
        &self,
        protocols: &[&str],
    ) -> Result<WebSocketStream, Error> {
        ensure_rustls_crypto_provider();
        Ok(connect_with_protocols(self.get_ws_url(), protocols).await?)
    }

    #[cfg(feature = "ws")]
    pub fn ws(&self, config: BoltzWsConfig) -> BoltzWsApi {
        BoltzWsApi::new(self.get_ws_url(), config)
    }

    /// Make a GET request. Returns the Response
    async fn get_response(&self, end_point: &str) -> Result<reqwest::Response, Error> {
        let url = format!("{}/{}", self.base_url, end_point);
        let req_builder = self.http_client.get(&url);
        let req_builder = self.maybe_add_timeout(req_builder);
        let req_builder = self.maybe_add_api_key(req_builder, &url)?;
        let response = req_builder.send().await?;
        self.reject_credential_leaking_redirect(&response, self.credentials_sent(None))?;
        Ok(response)
    }

    /// Make a GET request. Returns the Response as text
    async fn get(&self, end_point: &str) -> Result<String, Error> {
        Ok(self.get_response(end_point).await?.text().await?)
    }

    async fn get_json<T: DeserializeOwned>(&self, end_point: &str) -> Result<T, Error> {
        let response = self.get_response(end_point).await?;
        Self::parse_json_response(response).await
    }

    fn parse_value(body: String) -> Value {
        serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body))
    }

    async fn post_json<T: DeserializeOwned>(
        &self,
        end_point: &str,
        data: impl Serialize,
    ) -> Result<T, Error> {
        self.post_json_with_swap_auth(end_point, data, None).await
    }

    /// Same as [`Self::post_json`], with the per-swap taker credential in
    /// [`SWAP_AUTH_HEADER`] when the caller holds one.
    async fn post_json_with_swap_auth<T: DeserializeOwned>(
        &self,
        end_point: &str,
        data: impl Serialize,
        swap_auth: Option<&str>,
    ) -> Result<T, Error> {
        // `post_response` already checked the redirect: every credential this
        // client can send goes on in `request_response`, so that is the one place
        // that knows what a response might have carried away.
        let response = self.post_response(end_point, data, swap_auth).await?;
        Self::parse_json_response(response).await
    }

    /// The credential headers a request just went out with: the per-swap
    /// `swapAuth` when the caller passed one, and the organization API key when
    /// this client carries one.
    fn credentials_sent(&self, swap_auth: Option<&str>) -> Vec<&'static str> {
        let mut sent = Vec::new();
        if swap_auth.is_some() {
            sent.push(SWAP_AUTH_HEADER);
        }
        if self.api_key.is_some() {
            sent.push(API_KEY_HEADER);
        }
        sent
    }

    /// Fail a credential-bearing request that came back from a host other than
    /// the one it was addressed to.
    ///
    /// [`Self::default_http_client`] follows no redirects, so this cannot fire
    /// for a client built here. It can for a caller-supplied one
    /// ([`Self::with_client`]) and in the browser, where `fetch` owns redirect
    /// handling and reqwest sets no policy on it — a cross-origin hop needs a
    /// CORS preflight for [`SWAP_AUTH_HEADER`], but the host the `Location`
    /// names is free to answer that preflight itself. An *authenticated* client
    /// is narrower still: both [`KaleidoMakerClient`] constructors set
    /// `Policy::none()` themselves, so on native this is unreachable for one and
    /// the browser is the only case left.
    ///
    /// The hop has already happened by the time this runs, so this prevents
    /// nothing. What it does is refuse to hand back a response the maker did not
    /// send, and — where a credential went with it — make the disclosure
    /// visible, which is the difference between a credential the caller knows to
    /// treat as burnt and one they do not.
    ///
    /// It is a backstop and not the guarantee, because a [`reqwest::Response`]
    /// reports only the URL the chain *ended* at — no hop list, no `redirected`
    /// flag. A chain that detoured through another host and came back to the
    /// maker is therefore indistinguishable from no redirect at all, and
    /// [`SWAP_AUTH_HEADER`] would have ridden along to the detour. Only owning
    /// the redirect policy closes that, which is what
    /// [`Self::default_http_client`] and both [`KaleidoMakerClient`]
    /// constructors do.
    ///
    /// [`KaleidoMakerClient`]: crate::kaleido::KaleidoMakerClient
    ///
    /// The two credentials do not travel alike, and the message says which
    /// happened, because the reactions differ. [`SWAP_AUTH_HEADER`] is a custom
    /// header and rides along unconditionally, so it is always disclosed and the
    /// swap is always burnt.
    ///
    /// [`API_KEY_HEADER`] is stripped by the redirect — but on a *narrower* rule
    /// than the one this function calls an origin. `reqwest` drops it when the
    /// host or the port changes and does not look at the scheme at all
    /// (`redirect::remove_sensitive_headers`), so an `https` maker redirected to
    /// `http://same-host:443/…` re-sends the key **in cleartext**. That is the
    /// case [`same_origin`] catches and reqwest does not, and telling the partner
    /// there was nothing to revoke would be exactly wrong there. So the reaction
    /// is chosen from reqwest's rule, not from ours. Browsers are stricter — a
    /// scheme change is cross-origin to `fetch`, which drops `Authorization`
    /// with the rest of the CORS non-wildcard headers — so the advice is
    /// pessimistic there rather than wrong.
    fn reject_credential_leaking_redirect(
        &self,
        response: &reqwest::Response,
        carried: Vec<&'static str>,
    ) -> Result<(), Error> {
        if carried.is_empty() {
            return Ok(());
        }
        let sent_to = reqwest::Url::parse(&self.base_url)?;
        let came_from = response.url();
        if same_origin(&sent_to, came_from) {
            return Ok(());
        }

        // Named the way the destination differs, port included: a hop between
        // two ports on one host would otherwise read as a redirect to the host
        // it was already talking to, which reads like a bug in the SDK.
        let mut message = format!(
            "a request carrying {} was redirected to {}, which is not the maker it \
             was addressed to ({})",
            carried.join(" and "),
            came_from.origin().ascii_serialization(),
            sent_to.origin().ascii_serialization(),
        );
        if carried.contains(&SWAP_AUTH_HEADER) {
            message.push_str(&format!(
                "; {SWAP_AUTH_HEADER} is a custom header and follows a redirect, so \
                 the per-swap credential reached that host — treat it as disclosed \
                 and do not reuse the swap"
            ));
        }
        if carried.contains(&API_KEY_HEADER) {
            message.push_str(&api_key_redirect_advice(&sent_to, came_from));
        }
        Err(Error::Protocol(message))
    }

    async fn parse_json_response<T: DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, Error> {
        let status = response.status();
        let body = response.text().await?;
        Self::json_from_body(status, body)
    }

    /// Decide what a response body means, given the status it arrived with.
    ///
    /// The two failures are not the same failure. A non-success status is the
    /// maker rejecting the request, and its body — `invalid_swap_auth`,
    /// `pair_hash_mismatch` — is what makes the rejection diagnosable, so it is
    /// kept verbatim. A success status whose body does not deserialize means the
    /// request worked and the two sides disagree on the schema; that reports as
    /// [`Error::HTTPResponseBodyInvalid`], describing the mismatch without
    /// reproducing the body.
    fn json_from_body<T: DeserializeOwned>(
        status: reqwest::StatusCode,
        body: String,
    ) -> Result<T, Error> {
        if !status.is_success() {
            return Err(Error::HTTPStatusNotSuccess(status, Self::parse_value(body)));
        }

        match serde_json::from_str::<T>(&body) {
            Ok(parsed) => Ok(parsed),
            Err(e) => Err(Error::HTTPResponseBodyInvalid(
                status,
                Self::describe_parse_error(&body, &e),
            )),
        }
    }

    /// How many bytes of body key names [`Self::describe_parse_error`] will
    /// list. A create response carries about a dozen short field names; a
    /// server sending more than this is one we do not recognise, and the error
    /// should stay a log line rather than become a payload.
    const PARSE_ERROR_KEY_BUDGET: usize = 200;

    /// What replaces a value the body owned.
    const REDACTED: &'static str = "<redacted>";

    /// Describe a body that did not deserialize, without reproducing it.
    ///
    /// The body of a create response is not safe to log. The maker returns
    /// `swapAuth` — the per-swap taker credential — on every create, and sends
    /// it whether or not the SDK models the field, so a schema disagreement
    /// anywhere in the response would otherwise put the credential wherever this
    /// error goes: `Error::message()` formats it, the WebAssembly binding turns
    /// it into a JS `Error` message, and callers log those.
    ///
    /// What is kept is serde_json's own message, because it is the diagnosis —
    /// the missing field, the type that did not fit, a line/column into the
    /// body. What that message may also contain is a value serde read out of the
    /// body, and serde puts every such value in a delimited run:
    ///
    /// - A double-quoted run is a string serde echoed with `{:?}`, from an
    ///   `invalid type` or `invalid value`. Always a body value, so always
    ///   replaced.
    /// - A backticked run is either a name out of the schema — `missing field`,
    ///   `expected one of` — or the value of an unknown enum variant, which
    ///   serde renders identically. The body is what tells them apart, so a
    ///   backticked run is replaced exactly when the body carries that string as
    ///   a scalar. `SwapRestoreResponse::swap_type` makes this reachable, and a
    ///   maker credential landing in an enum-typed field would otherwise have
    ///   survived verbatim.
    ///
    /// Prose outside a delimited run is never touched, so nothing can mangle the
    /// diagnosis. Then come the body's top-level keys, which serde does not
    /// report for a type mismatch: names are the schema under dispute, and the
    /// values are the secret.
    fn describe_parse_error(body: &str, e: &serde_json::Error) -> String {
        // One parse, shared by both halves. `None` when the body is not JSON at
        // all, in which case serde's error is a syntax error and carries no
        // body content to redact.
        let body = serde_json::from_str::<Value>(body).ok();

        let mut scalars = HashSet::new();
        if let Some(body) = &body {
            Self::collect_scalars(body, &mut scalars);
        }
        let mut described = Self::redact_body_values(&e.to_string(), &scalars);

        if let Some(keys) = body.as_ref().and_then(Self::top_level_keys) {
            described.push_str("; body keys: ");
            described.push_str(&keys);
        }

        described
    }

    /// Every scalar the body carries, rendered the way serde renders it in a
    /// message. Object keys are deliberately excluded: a key is a name from the
    /// schema under dispute, which [`Self::describe_parse_error`] reports on
    /// purpose. `serde_json` caps its own recursion when building a [`Value`],
    /// so the depth here is bounded by the parse that produced it.
    fn collect_scalars(body: &Value, into: &mut HashSet<String>) {
        match body {
            Value::Object(fields) => fields.values().for_each(|v| Self::collect_scalars(v, into)),
            Value::Array(items) => items.iter().for_each(|v| Self::collect_scalars(v, into)),
            Value::String(s) => {
                into.insert(s.clone());
            }
            Value::Number(n) => {
                into.insert(n.to_string());
            }
            Value::Bool(b) => {
                into.insert(b.to_string());
            }
            Value::Null => {}
        }
    }

    /// Replace the body's own values where serde quoted them back, leaving the
    /// rest of its message alone. See [`Self::describe_parse_error`] for which
    /// runs are body values and why.
    fn redact_body_values(message: &str, scalars: &HashSet<String>) -> String {
        let mut out = String::with_capacity(message.len());
        let mut run = String::new();
        let mut delimiter = None;
        let mut escaped = false;

        for c in message.chars() {
            let Some(opened) = delimiter else {
                out.push(c);
                if c == '"' || c == '`' {
                    delimiter = Some(c);
                    run.clear();
                }
                continue;
            };

            // Only `{:?}` escapes, and only inside the run it wrote, so a `\"`
            // there is part of the value rather than its end.
            if opened == '"' && escaped {
                escaped = false;
                run.push(c);
            } else if opened == '"' && c == '\\' {
                escaped = true;
            } else if c == opened {
                if opened == '"' || scalars.contains(&run) {
                    out.push_str(Self::REDACTED);
                } else {
                    out.push_str(&run);
                }
                out.push(c);
                delimiter = None;
            } else {
                run.push(c);
            }
        }

        // A run serde never closed is a value of unknown extent. Drop it.
        if delimiter.is_some() {
            out.push_str(Self::REDACTED);
        }

        out
    }

    /// The body's top-level object keys, sorted and bounded by
    /// [`Self::PARSE_ERROR_KEY_BUDGET`]. `None` when there are none to report:
    /// an empty object, an array, or a scalar.
    fn top_level_keys(body: &Value) -> Option<String> {
        let mut keys: Vec<&str> = body.as_object()?.keys().map(String::as_str).collect();
        // Deterministic regardless of how `serde_json::Map` is ordered.
        keys.sort_unstable();

        let mut listed = String::new();
        let mut omitted = 0usize;
        for key in keys {
            if listed.len() + key.len() > Self::PARSE_ERROR_KEY_BUDGET {
                omitted += 1;
                continue;
            }
            if !listed.is_empty() {
                listed.push_str(", ");
            }
            listed.push_str(key);
        }

        if listed.is_empty() {
            return None;
        }
        if omitted > 0 {
            listed.push_str(&format!(" (+{omitted} more)"));
        }

        Some(listed)
    }

    /// Make a POST request. Returns the Response
    async fn post_response(
        &self,
        end_point: &str,
        data: impl Serialize,
        swap_auth: Option<&str>,
    ) -> Result<reqwest::Response, Error> {
        let url = format!("{}/{}", self.base_url, end_point);
        self.request_response(Method::POST, url, data, swap_auth)
            .await
    }

    /// Make a PATCH request. Returns the Response
    async fn patch(&self, end_point: &str, data: impl Serialize) -> Result<Value, Error> {
        let url = format!("{}/{}", self.base_url, end_point);

        self.request(Method::PATCH, url, data).await
    }

    async fn request(
        &self,
        method: Method,
        url: String,
        data: impl Serialize,
    ) -> Result<Value, Error> {
        let response = self
            .request_response(method.clone(), url, data, None)
            .await?;
        Self::parse_json_response(response).await
    }

    async fn request_response(
        &self,
        method: Method,
        url: String,
        data: impl Serialize,
        swap_auth: Option<&str>,
    ) -> Result<reqwest::Response, Error> {
        let method_str = method.to_string();
        let req_builder = self.http_client.request(method, &url).json(&data);
        let req_builder = self.maybe_add_timeout(req_builder);
        let req_builder = Self::maybe_add_swap_auth(req_builder, swap_auth)?;
        let req_builder = self.maybe_add_api_key(req_builder, &url)?;
        match req_builder.send().await {
            Ok(response) => {
                self.reject_credential_leaking_redirect(
                    &response,
                    self.credentials_sent(swap_auth),
                )?;
                Ok(response)
            }
            Err(e) => {
                log::error!("{method_str} error: {e:#?}");
                Err(e.into())
            }
        }
    }

    fn maybe_add_timeout(&self, req_builder: RequestBuilder) -> RequestBuilder {
        if let Some(timeout) = self.timeout {
            req_builder.timeout(timeout)
        } else {
            req_builder
        }
    }

    /// Attach the partner organization API key — and only to the exact origin
    /// it was configured for.
    ///
    /// The key names an organization to the KaleidoSwap maker and to nobody
    /// else. Esplora, the Platform API, a merchant webhook and a second maker
    /// are all hosts that would learn a permanent credential they have no use
    /// for, and any one of them could then attribute its own swaps to that
    /// organization. Every URL this client builds is `base_url` plus a path, so
    /// the comparison holds structurally; it is written out anyway, because
    /// "the code currently only builds URLs one way" is not a property the type
    /// system is checking, and a mismatch here means the key was about to go
    /// somewhere it was never meant to.
    fn maybe_add_api_key(
        &self,
        req_builder: RequestBuilder,
        url: &str,
    ) -> Result<RequestBuilder, Error> {
        let Some(api_key) = &self.api_key else {
            return Ok(req_builder);
        };
        let configured = reqwest::Url::parse(&self.base_url)?;
        let target = reqwest::Url::parse(url)?;
        if !same_origin(&configured, &target) {
            return Err(Error::Protocol(format!(
                "refusing to send organization API key {} to {} — it is bound to {}",
                api_key.redacted(),
                target.host_str().unwrap_or(url),
                configured.host_str().unwrap_or(&self.base_url),
            )));
        }
        Ok(req_builder.header(API_KEY_HEADER, api_key.bearer_header_value()?))
    }

    /// Attach the per-swap taker credential, rejecting a value that cannot be
    /// a header before it reaches the wire.
    ///
    /// `reqwest` would otherwise swallow the malformed value and surface it as
    /// a generic send failure at the end of the request, which reads like the
    /// maker refused the swap rather than like a bad credential — the one
    /// distinction a caller debugging a stored `swapAuth` needs. An empty
    /// string is rejected too: it is a well-formed header the maker can only
    /// answer `401`, so failing here says what actually went wrong.
    fn maybe_add_swap_auth(
        req_builder: RequestBuilder,
        swap_auth: Option<&str>,
    ) -> Result<RequestBuilder, Error> {
        let Some(swap_auth) = swap_auth else {
            return Ok(req_builder);
        };
        if swap_auth.is_empty() {
            return Err(Error::Protocol(format!(
                "swap auth credential is empty — pass no credential at all for a \
                 maker that issues none, rather than an empty {SWAP_AUTH_HEADER}"
            )));
        }
        let mut value = HeaderValue::from_str(swap_auth).map_err(|_| {
            Error::Protocol(format!(
                "swap auth credential is not a valid {SWAP_AUTH_HEADER} value"
            ))
        })?;
        // Keeps the credential out of hyper's header dumps and, over HTTP/2,
        // out of the HPACK dynamic table that a connection-level observer or a
        // later request on the same connection could otherwise index it into.
        value.set_sensitive(true);
        Ok(req_builder.header(SWAP_AUTH_HEADER, value))
    }

    pub async fn get_fee_estimation(&self) -> Result<GetFeeEstimationResponse, Error> {
        self.get_json("chain/fees").await
    }

    pub async fn get_height(&self) -> Result<HeightResponse, Error> {
        self.get_json("chain/heights").await
    }

    pub async fn get_submarine_pairs(&self) -> Result<GetSubmarinePairsResponse, Error> {
        self.get_json("swap/submarine").await
    }

    pub async fn get_reverse_pairs(&self) -> Result<GetReversePairsResponse, Error> {
        self.get_json("swap/reverse").await
    }

    pub async fn get_chain_pairs(&self) -> Result<GetChainPairsResponse, Error> {
        self.get_json("swap/chain").await
    }

    pub async fn post_swap_req(
        &self,
        swap_request: &CreateSubmarineRequest,
    ) -> Result<CreateSubmarineResponse, Error> {
        let data = serde_json::to_value(swap_request)?;
        self.post_json("swap/submarine", data).await
    }

    pub async fn post_reverse_req(
        &self,
        req: CreateReverseRequest,
    ) -> Result<CreateReverseResponse, Error> {
        self.post_json("swap/reverse", req).await
    }

    pub async fn post_chain_req(
        &self,
        req: CreateChainRequest,
    ) -> Result<CreateChainResponse, Error> {
        self.post_json("swap/chain", req).await
    }

    pub async fn get_submarine_claim_tx_details(
        &self,
        id: &String,
    ) -> Result<SubmarineClaimTxResponse, Error> {
        let endpoint = format!("swap/submarine/{id}/claim");
        self.get_json(&endpoint).await
    }

    pub async fn get_chain_claim_tx_details(
        &self,
        id: &String,
    ) -> Result<Option<ChainClaimTxResponse>, Error> {
        let endpoint = format!("swap/chain/{id}/claim");
        let res = self.get(&endpoint).await?;

        match serde_json::from_str(&res) {
            Ok(response) => Ok(response),
            Err(e) => {
                let error: ErrorResponse = serde_json::from_str(&res)?;
                if error.error == "server claim succeeded already" {
                    Ok(None)
                } else {
                    Err(Error::JSON(e))
                }
            }
        }
    }

    pub async fn post_submarine_claim_tx_details(
        &self,
        id: &String,
        pub_nonce: musig::PublicNonce,
        partial_sig: musig::PartialSignature,
    ) -> Result<Value, Error> {
        let data = json!(
            {
                "pubNonce": pub_nonce.serialize().to_lower_hex_string(),
                "partialSignature": partial_sig.serialize().to_lower_hex_string()
            }
        );
        let endpoint = format!("swap/submarine/{id}/claim");
        self.post_json(&endpoint, data).await
    }

    pub async fn post_chain_claim_tx_details(
        &self,
        id: &String,
        preimage: &Preimage,
        signature: Option<(musig::PartialSignature, musig::PublicNonce)>,
        to_sign: ToSign,
    ) -> Result<PartialSig, Error> {
        let data = match signature {
            Some((partial_sig, pub_nonce)) => json!(
                {
                "preimage": preimage.bytes.ok_or(Error::Protocol("Preimage bytes not available to post chain claim".to_string()))?.to_lower_hex_string(),
                "signature": PartialSig {
                    pub_nonce: pub_nonce.serialize().to_lower_hex_string(),
                    partial_signature: partial_sig.serialize().to_lower_hex_string(),
                },
                "toSign": to_sign,
            }
            ),
            None => json!(
                {
                    "preimage": preimage.bytes.ok_or(Error::Protocol("Preimage bytes not available to post chain claim".to_string()))?.to_lower_hex_string(),
                    "toSign": to_sign,
                }
            ),
        };
        let endpoint = format!("swap/chain/{id}/claim");
        self.post_json(&endpoint, data).await
    }

    pub async fn get_reverse_tx(&self, id: &str) -> Result<ReverseSwapTxResp, Error> {
        self.get_json(&format!("swap/reverse/{id}/transaction"))
            .await
    }

    pub async fn get_submarine_tx(&self, id: &str) -> Result<SubmarineSwapTxResp, Error> {
        self.get_json(&format!("swap/submarine/{id}/transaction"))
            .await
    }

    pub async fn get_submarine_preimage(
        &self,
        id: &str,
    ) -> Result<SubmarineSwapPreimageResp, Error> {
        self.get_json(&format!("swap/submarine/{id}/preimage"))
            .await
    }

    pub async fn get_chain_txs(&self, id: &str) -> Result<ChainSwapTxResp, Error> {
        self.get_json(&format!("swap/chain/{id}/transactions"))
            .await
    }

    pub async fn get_reverse_partial_sig(
        &self,
        id: &String,
        preimage: &Preimage,
        pub_nonce: &musig::PublicNonce,
        claim_tx_hex: &String,
    ) -> Result<PartialSig, Error> {
        let data = json!(
            {
                "preimage": preimage.bytes.ok_or(Error::Protocol("Preimage bytes not available to post chain claim".to_string()))?.to_lower_hex_string(),
                "pubNonce": pub_nonce.serialize().to_lower_hex_string(),
                "transaction": claim_tx_hex,
                "index": 0
            }
        );

        let endpoint = format!("swap/reverse/{id}/claim");
        self.post_json(&endpoint, data).await
    }

    pub async fn get_submarine_partial_sig(
        &self,
        id: &String,
        input_index: usize,
        pub_nonce: &musig::PublicNonce,
        refund_tx_hex: &String,
    ) -> Result<PartialSig, Error> {
        let data = json!(
            {
                "pubNonce": pub_nonce.serialize().to_lower_hex_string(),
                "transaction": refund_tx_hex,
                "index": input_index
            }
        );

        let endpoint = format!("swap/submarine/{id}/refund");
        self.post_json(&endpoint, data).await
    }

    pub async fn get_chain_partial_sig(
        &self,
        id: &String,
        input_index: usize,
        pub_nonce: &musig::PublicNonce,
        refund_tx_hex: &String,
    ) -> Result<PartialSig, Error> {
        let data = json!(
            {
                "pubNonce": pub_nonce.serialize().to_lower_hex_string(),
                "transaction": refund_tx_hex,
                "index": input_index
            }
        );

        let endpoint = format!("swap/chain/{id}/refund");
        self.post_json(&endpoint, data).await
    }

    pub async fn get_mrh_bip21(&self, invoice: &str) -> Result<MrhResponse, Error> {
        let request = format!("swap/reverse/{invoice}/bip21");
        self.get_json(&request).await
    }

    pub async fn broadcast_tx(&self, chain: Chain, tx_hex: &String) -> Result<Value, Error> {
        let data = json!(
            {
                "hex": tx_hex
            }
        );

        let chain = match chain {
            Chain::Bitcoin(_) => "BTC",
            Chain::Liquid(_) => "L-BTC",
        };

        let end_point = format!("chain/{chain}/transaction");
        self.post_json(&end_point, data).await
    }

    /// Creates a BOLT12 offer
    pub async fn post_bolt12_offer(&self, req: CreateBolt12OfferRequest) -> Result<(), Error> {
        let data = serde_json::to_value(req)?;
        let end_point = "lightning/BTC/bolt12".to_string();
        self.post_json::<Value>(&end_point, data).await?;
        Ok(())
    }

    /// Updates the webhook URL for a BOLT12 offer
    ///
    /// # Arguments
    ///   * `req` - The request object containing the offer and the new webhook URL
    ///     * `offer` - The BOLT12 offer
    ///     * `url` - The updated webhook URL. Setting to None will remove the webhook URL from the registered offer
    ///     * `signature` - The schnorr signature of the SHA256 hash of the webhook URL or "UPDATE" when not set
    pub async fn patch_bolt12_offer(&self, req: UpdateBolt12OfferRequest) -> Result<(), Error> {
        let data = serde_json::to_value(req)?;
        let end_point = "lightning/BTC/bolt12".to_string();
        self.patch(&end_point, data).await?;
        Ok(())
    }

    /// Deletes a BOLT12 offer
    ///
    /// # Arguments
    ///    * `offer` - The BOLT12 offer
    ///    * `signature` - This schnorr signature of the SHA256 hash of "DELETE"
    pub async fn delete_bolt12_offer(&self, offer: &str, signature: &str) -> Result<(), Error> {
        let data = json!(
            {
                "offer": offer,
                "signature": signature,
            }
        );

        let end_point = "lightning/BTC/bolt12/delete".to_string();
        self.post_json::<Value>(&end_point, data).await?;
        Ok(())
    }

    /// Fetch an invoice for the specified BOLT12 offer
    pub async fn get_bolt12_invoice(
        &self,
        req: GetBolt12FetchRequest,
    ) -> Result<GetBolt12FetchResponse, Error> {
        let data = serde_json::to_value(req)?;
        let end_point = "lightning/BTC/bolt12/fetch".to_string();
        self.post_json(&end_point, data).await
    }

    /// Gets parameters for a BOLT12 offer
    pub async fn get_bolt12_params(&self) -> Result<GetBolt12ParamsResponse, Error> {
        let end_point = "lightning/BTC/bolt12/L-BTC".to_string();
        self.get_json(&end_point).await
    }

    /// Fetch information about the Lightning nodes the backend is connected to
    pub async fn get_nodes(&self) -> Result<GetNodesResponse, Error> {
        let end_point = "nodes".to_string();
        self.get_json(&end_point).await
    }

    /// Gets a quote for a Zero-Amount or over- or underpaid Chain Swap.
    ///
    /// If the user locked up a valid amount, it will return the server lockup amount. In all other
    /// cases, it will return an error.
    ///
    /// Needs no `swapAuth`: it reads a proposal the maker itself published and
    /// commits nothing. Only [`Self::accept_quote`] is gated, so seeing a
    /// re-quote here says nothing about being able to accept it.
    pub async fn get_quote(&self, swap_id: &str) -> Result<GetQuoteResponse, Error> {
        let end_point = format!("swap/chain/{swap_id}/quote");
        self.get_json(&end_point).await
    }

    /// Accepts a specific quote for a Zero-Amount or over- or underpaid Chain Swap.
    ///
    /// `swap_auth` is the per-swap taker credential the KaleidoSwap maker
    /// returned as `swapAuth` when the swap was created — see
    /// [`CreateChainResponse::swap_auth`]. Accepting a re-quote commits the
    /// maker's payout at the re-quoted amount, so the maker authorizes it with
    /// the credential rather than with the swap id: the id travels through
    /// status polls, `/v2/ws`, webhooks and logs, and anyone who saw one could
    /// otherwise settle — or refuse and force a refund on — a stranger's live
    /// swap. Without the credential the KaleidoSwap maker answers
    /// `401 invalid_swap_auth`, and no other route resolves the re-quote: the
    /// swap sits until it expires into its refund path.
    ///
    /// Pass `None` for a maker that issues no credential (upstream Boltz
    /// declares no auth on this route). A stored `swapAuth` for a swap created
    /// against KaleidoSwap must be passed, and cannot be recovered from the
    /// SDK: `POST /v2/swap/restore` does not re-issue it, so a lost credential
    /// is an operator recovery, not a client one.
    pub async fn accept_quote(
        &self,
        swap_id: &str,
        amount_sat: u64,
        swap_auth: Option<&str>,
    ) -> Result<(), Error> {
        let data = json!(
            {
                "amount": amount_sat
            }
        );

        let end_point = format!("swap/chain/{swap_id}/quote");
        self.post_json_with_swap_auth::<Value>(&end_point, data, swap_auth)
            .await?;
        Ok(())
    }

    /// Gets the latest status of the Swap
    pub async fn get_swap(&self, swap_id: &str) -> Result<GetSwapResponse, Error> {
        let end_point = format!("swap/{swap_id}");
        self.get_json(&end_point).await
    }

    /// Restore swaps from an xpub.
    ///
    /// `derivation_path` is the path boltz appends `/{index}` to when deriving
    /// child keys from `xpub`. Pass `"m"` when `xpub` is already the
    /// swap-account key (`m/44/0/0/0`), so boltz derives `xpub/{index}` to match
    /// our per-swap keys. Omitting the path makes boltz apply its own default
    /// and find nothing.
    pub async fn post_swap_restore(
        &self,
        xpub: &String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<Vec<SwapRestoreResponse>, Error> {
        let mut data = json!({ "xpub": xpub });
        if let Some(path) = derivation_path {
            data["derivationPath"] = json!(path);
        }
        if let Some(gap) = gap_limit {
            data["gapLimit"] = json!(gap);
        }

        self.post_json("swap/restore", data).await
    }

    /// Highest swap-key derivation index boltz has seen for `xpub` (-1 if none).
    /// See [`Self::post_swap_restore`] for the `derivation_path` semantics.
    pub async fn post_swap_restore_index(
        &self,
        xpub: &String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<SwapRestoreIndexResponse, Error> {
        let mut data = json!({ "xpub": xpub });
        if let Some(path) = derivation_path {
            data["derivationPath"] = json!(path);
        }
        if let Some(gap) = gap_limit {
            data["gapLimit"] = json!(gap);
        }

        self.post_json("swap/restore/index", data).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainClaimTxResponse {
    pub pub_nonce: String,
    pub public_key: PublicKey,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmarineClaimTxResponse {
    pub preimage: String,
    pub pub_nonce: String,
    pub public_key: PublicKey,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrhResponse {
    pub bip21: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook<T> {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_swap_id: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubmarineRequest {
    pub from: String,
    pub to: String,
    pub invoice: String,
    pub refund_public_key: PublicKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<Webhook<SubSwapStates>>,
}

/// Renders a `swap_auth` field for `Debug` without printing it.
///
/// The credential is the taker's full capability over a swap, so a caller that
/// logs a whole create response — `log::debug!("{resp:?}")` — must not thereby
/// log the credential. Whether the maker issued one is still worth seeing, so
/// only the value is withheld.
struct RedactedSwapAuth<'a>(&'a Option<String>);

impl std::fmt::Debug for RedactedSwapAuth<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(_) => f.write_str("Some(<redacted>)"),
            None => f.write_str("None"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubmarineResponse {
    pub accept_zero_conf: bool,
    pub address: String,
    pub bip21: String,
    pub claim_public_key: PublicKey,
    pub expected_amount: u64,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_id: Option<String>,
    pub swap_tree: SwapTree,
    pub timeout_block_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blinding_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
    /// Per-swap taker credential, returned **once** on creation by the
    /// KaleidoSwap maker. No submarine-swap route needs it today; it is captured
    /// so a caller can persist it with the swap rather than lose it. See
    /// [`CreateChainResponse::swap_auth`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_auth: Option<String>,
}
/// Hand-written only to redact `swap_auth`; every other field prints as the
/// derive would. The exhaustive `let Self { .. }` is load-bearing — a field
/// added to the struct later fails to compile here rather than silently going
/// missing from the output.
impl std::fmt::Debug for CreateSubmarineResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            accept_zero_conf,
            address,
            bip21,
            claim_public_key,
            expected_amount,
            id,
            referral_id,
            swap_tree,
            timeout_block_height,
            blinding_key,
            asset_id,
            fee_asset_id,
            swap_auth,
        } = self;
        f.debug_struct("CreateSubmarineResponse")
            .field("accept_zero_conf", accept_zero_conf)
            .field("address", address)
            .field("bip21", bip21)
            .field("claim_public_key", claim_public_key)
            .field("expected_amount", expected_amount)
            .field("id", id)
            .field("referral_id", referral_id)
            .field("swap_tree", swap_tree)
            .field("timeout_block_height", timeout_block_height)
            .field("blinding_key", blinding_key)
            .field("asset_id", asset_id)
            .field("fee_asset_id", fee_asset_id)
            .field("swap_auth", &RedactedSwapAuth(swap_auth))
            .finish()
    }
}
impl CreateSubmarineResponse {
    /// Ensure submarine swap redeem script uses the preimage hash used in the invoice
    pub fn validate(
        &self,
        invoice: &str,
        our_pubkey: &PublicKey,
        chain: Chain,
    ) -> Result<(), Error> {
        self.validate_with_currency(invoice, our_pubkey, chain, None)
    }

    pub fn validate_with_currency(
        &self,
        invoice: &str,
        our_pubkey: &PublicKey,
        chain: Chain,
        currency: Option<Currency>,
    ) -> Result<(), Error> {
        self.validate_with_currency_and_asset_context(invoice, our_pubkey, chain, currency, None)
    }

    /// Validate a create response against both the requested currency and the
    /// asset ids from the pair card the caller accepted.
    pub fn validate_with_currency_and_asset_context(
        &self,
        invoice: &str,
        our_pubkey: &PublicKey,
        chain: Chain,
        currency: Option<Currency>,
        expected_asset_context: Option<LiquidAssetContext>,
    ) -> Result<(), Error> {
        let preimage = Preimage::from_invoice_str(invoice)?;

        match chain {
            Chain::Bitcoin(bitcoin_chain) => {
                let boltz_sub_script = BtcSwapScript::submarine_from_swap_resp(self, *our_pubkey)?;
                boltz_sub_script.validate_address(bitcoin_chain, self.address.clone())
            }
            Chain::Liquid(liquid_chain) => {
                let boltz_sub_script =
                    LiquidSwapScript::submarine_from_swap_resp(self, *our_pubkey)?;
                boltz_sub_script.validate_currency(
                    liquid_chain,
                    chain.resolve_currency(currency)?,
                    expected_asset_context,
                )?;
                if boltz_sub_script.hashlock != preimage.hash160 {
                    return Err(Error::Protocol(format!(
                        "Hash160 mismatch: {},{}",
                        boltz_sub_script.hashlock, preimage.hash160
                    )));
                }

                boltz_sub_script.validate_address(liquid_chain, self.address.clone())
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapTree {
    pub claim_leaf: Leaf,
    pub refund_leaf: Leaf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leaf {
    pub output: String,
    pub version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimDetails {
    pub tree: SwapTree,
    pub amount: Option<u64>,
    pub key_index: u32,
    pub transaction: Option<TransactionOut>,
    pub lockup_address: String,
    pub server_public_key: String,
    pub timeout_block_height: u32,
    pub blinding_key: Option<String>,
    pub preimage_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundDetails {
    pub tree: SwapTree,
    pub key_index: u32,
    pub transaction: Option<TransactionOut>,
    pub lockup_address: String,
    pub server_public_key: String,
    pub timeout_block_height: u32,
    pub blinding_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwapRestoreType {
    Reverse,
    Submarine,
    Chain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRestoreResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub swap_type: SwapRestoreType,
    pub status: String,
    pub created_at: u64,
    pub from: String,
    pub to: String,
    /// Lightning invoice; boltz only returns it for submarine/reverse swaps.
    pub invoice: Option<String>,
    pub claim_details: Option<ClaimDetails>,
    pub refund_details: Option<RefundDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRestoreIndexResponse {
    pub index: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SubscriptionChannel {
    #[serde(rename = "swap.update")]
    SwapUpdate,
    #[serde(rename = "invoice.request")]
    InvoiceRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InvoiceRequestParams {
    pub offer: String,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "channel")]
pub enum SubscribeRequest {
    #[serde(rename = "swap.update")]
    SwapUpdate { args: Vec<String> },
    #[serde(rename = "invoice.request")]
    InvoiceRequest { args: Vec<InvoiceRequestParams> },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct UnsubscribeRequest {
    pub channel: SubscriptionChannel,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InvoiceCreated {
    pub id: String,
    pub invoice: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InvoiceError {
    pub id: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum WsRequest {
    #[serde(rename = "subscribe")]
    Subscribe(SubscribeRequest),
    #[serde(rename = "unsubscribe")]
    Unsubscribe(UnsubscribeRequest),
    #[serde(rename = "invoice")]
    Invoice(InvoiceCreated),
    #[serde(rename = "invoice.error")]
    InvoiceError(InvoiceError),
    #[serde(rename = "ping")]
    Ping,
}

impl WsRequest {
    pub fn subscribe_swap_request(swap_id: &str) -> Self {
        Self::subscribe_swaps_request(vec![swap_id.to_string()])
    }

    pub fn subscribe_swaps_request(swap_ids: Vec<String>) -> Self {
        Self::Subscribe(SubscribeRequest::SwapUpdate { args: swap_ids })
    }

    pub fn subscribe_invoice_request(params: InvoiceRequestParams) -> Self {
        Self::subscribe_invoice_requests(vec![params])
    }

    pub fn subscribe_invoice_requests(params: Vec<InvoiceRequestParams>) -> Self {
        Self::Subscribe(SubscribeRequest::InvoiceRequest { args: params })
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct SubscribeResponse {
    pub channel: SubscriptionChannel,
    pub args: Vec<String>,

    pub timestamp: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct UnsubscribeResponse {
    pub channel: SubscriptionChannel,
    pub args: Vec<String>,

    pub timestamp: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct TransactionInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct FailureReasonIncorrectAmounts {
    pub expected: u64,
    pub actual: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ChannelInfo {
    #[serde(rename = "fundingTransactionId")]
    pub funding_transaction_id: String,
    #[serde(rename = "fundingTransactionVout")]
    pub funding_transaction_vout: u64,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct SwapStatus {
    pub id: String,
    pub status: String,

    #[serde(rename = "zeroConfRejected", skip_serializing_if = "Option::is_none")]
    pub zero_conf_rejected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction: Option<TransactionInfo>,

    #[serde(rename = "failureReason", skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(rename = "failureDetails", skip_serializing_if = "Option::is_none")]
    pub failure_details: Option<FailureReasonIncorrectAmounts>,

    #[serde(rename = "channel", skip_serializing_if = "Option::is_none")]
    pub channel_info: Option<ChannelInfo>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct InvoiceRequest {
    pub id: String,

    pub offer: String,
    #[serde(rename = "invoiceRequest")]
    pub invoice_request: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct UpdateResponse<T> {
    pub channel: SubscriptionChannel,
    pub args: Vec<T>,

    pub timestamp: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(tag = "event")]
pub enum WsResponse {
    #[serde(rename = "subscribe")]
    Subscribe(SubscribeResponse),
    #[serde(rename = "unsubscribe")]
    Unsubscribe(UnsubscribeResponse),
    #[serde(rename = "update")]
    Update(UpdateResponse<SwapStatus>),
    #[serde(rename = "request")]
    InvoiceRequest(UpdateResponse<InvoiceRequest>),
    #[serde(rename = "error")]
    Error(ErrorResponse),
    #[serde(rename = "pong")]
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReverseRequest {
    pub from: String,
    pub to: String,
    pub claim_public_key: PublicKey,
    /// The BOLT12 invoice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
    /// The invoice amount if the invoice is not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_amount: Option<u64>,
    /// The preimage hash if the invoice is not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage_hash: Option<sha256::Hash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<Webhook<RevSwapStates>>,
    /// Hash of the rate card the caller priced against, as
    /// [`CreateSubmarineRequest::pair_hash`] and
    /// [`CreateChainRequest::pair_hash`] already carry.
    ///
    /// Without it the maker cannot check the rate the caller agreed to, so a
    /// reverse swap is created at whatever the rate happens to be now — the
    /// one route of the three where the rate lock silently did nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_hash: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReverseResponse {
    pub id: String,
    pub invoice: Option<String>,
    pub swap_tree: SwapTree,
    pub lockup_address: String,
    pub refund_public_key: PublicKey,
    pub timeout_block_height: u32,
    pub onchain_amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blinding_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
    /// Per-swap taker credential, returned **once** on creation by the
    /// KaleidoSwap maker. No reverse-swap route needs it today; it is captured
    /// so a caller can persist it with the swap rather than lose it. See
    /// [`CreateChainResponse::swap_auth`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_auth: Option<String>,
}
/// Hand-written only to redact `swap_auth`; every other field prints as the
/// derive would. The exhaustive `let Self { .. }` is load-bearing — a field
/// added to the struct later fails to compile here rather than silently going
/// missing from the output.
impl std::fmt::Debug for CreateReverseResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            invoice,
            swap_tree,
            lockup_address,
            refund_public_key,
            timeout_block_height,
            onchain_amount,
            blinding_key,
            asset_id,
            fee_asset_id,
            swap_auth,
        } = self;
        f.debug_struct("CreateReverseResponse")
            .field("id", id)
            .field("invoice", invoice)
            .field("swap_tree", swap_tree)
            .field("lockup_address", lockup_address)
            .field("refund_public_key", refund_public_key)
            .field("timeout_block_height", timeout_block_height)
            .field("onchain_amount", onchain_amount)
            .field("blinding_key", blinding_key)
            .field("asset_id", asset_id)
            .field("fee_asset_id", fee_asset_id)
            .field("swap_auth", &RedactedSwapAuth(swap_auth))
            .finish()
    }
}
impl CreateReverseResponse {
    /// Validate reverse swap response
    /// Ensure reverse swap invoice uses the provided preimage
    /// Ensure reverse swap redeem script matches locally constructured SwapScript
    pub fn validate(
        &self,
        preimage: &Preimage,
        our_pubkey: &PublicKey,
        chain: Chain,
    ) -> Result<(), Error> {
        self.validate_with_currency(preimage, our_pubkey, chain, None)
    }

    pub fn validate_with_currency(
        &self,
        preimage: &Preimage,
        our_pubkey: &PublicKey,
        chain: Chain,
        currency: Option<Currency>,
    ) -> Result<(), Error> {
        self.validate_with_currency_and_asset_context(preimage, our_pubkey, chain, currency, None)
    }

    /// Validate a create response against both the requested currency and the
    /// asset ids from the pair card the caller accepted.
    pub fn validate_with_currency_and_asset_context(
        &self,
        preimage: &Preimage,
        our_pubkey: &PublicKey,
        chain: Chain,
        currency: Option<Currency>,
        expected_asset_context: Option<LiquidAssetContext>,
    ) -> Result<(), Error> {
        if let Some(invoice) = &self.invoice {
            // Boltz will only return a BOLT11 invoice if the invoice is not provided
            let invoice = Bolt11Invoice::from_str(invoice)?;
            if invoice.payment_hash().to_string() != preimage.sha256.to_string() {
                return Err(Error::Protocol(format!(
                    "Preimage hash mismatch : {},{}",
                    &invoice.payment_hash().to_string(),
                    preimage.sha256
                )));
            }
        }

        match chain {
            Chain::Bitcoin(bitcoin_chain) => {
                let boltz_rev_script = BtcSwapScript::reverse_from_swap_resp(self, *our_pubkey)?;
                boltz_rev_script.validate_address(bitcoin_chain, self.lockup_address.clone())
            }
            Chain::Liquid(liquid_chain) => {
                let boltz_rev_script = LiquidSwapScript::reverse_from_swap_resp(self, *our_pubkey)?;
                boltz_rev_script.validate_currency(
                    liquid_chain,
                    chain.resolve_currency(currency)?,
                    expected_asset_context,
                )?;
                boltz_rev_script.validate_address(liquid_chain, self.lockup_address.clone())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Lockup,
    Claim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSwapDetails {
    pub swap_tree: SwapTree,
    pub lockup_address: String,
    pub server_public_key: PublicKey,
    pub timeout_block_height: u32,
    pub amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blinding_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bip21: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChainRequest {
    pub from: String,
    pub to: String,
    pub preimage_hash: sha256::Hash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_public_key: Option<PublicKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_public_key: Option<PublicKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_lock_amount: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_lock_amount: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<Webhook<ChainSwapStates>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChainResponse {
    pub id: String,
    pub claim_details: ChainSwapDetails,
    pub lockup_details: ChainSwapDetails,
    /// Per-swap taker credential, returned **once** on creation by the
    /// KaleidoSwap maker and required to accept a chain re-quote — pass it to
    /// [`BoltzApiClientV2::accept_quote`], which sends it in
    /// [`SWAP_AUTH_HEADER`].
    ///
    /// It is `HMAC-SHA256` of the swap id under a key only the maker holds,
    /// and it is the taker's full capability over that swap: treat it as secret
    /// material, and persist it alongside the swap so a re-quote created in one
    /// session can still be accepted in the next. Nothing re-issues it —
    /// `POST /v2/swap/restore` authenticates with an XPUB alone and does not
    /// hand it back, so losing it means the swap can only run out its refund
    /// path unless an operator recovers the credential.
    ///
    /// `None` against a maker that issues none: this is a KaleidoSwap
    /// extension, and upstream Boltz declares no auth on the accept route.
    ///
    /// `Debug` on these responses redacts it, so `{:?}` on a whole create
    /// response is safe to log. The generated UniFFI `__str__` does not — that
    /// is generated code, and dropping the field from the FFI surface would
    /// take the credential away from the Python, Kotlin and Swift callers who
    /// need to persist it — so on those bindings log the swap id instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_auth: Option<String>,
}
/// Hand-written only to redact `swap_auth`; every other field prints as the
/// derive would. The exhaustive `let Self { .. }` is load-bearing — a field
/// added to the struct later fails to compile here rather than silently going
/// missing from the output.
impl std::fmt::Debug for CreateChainResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            claim_details,
            lockup_details,
            swap_auth,
        } = self;
        f.debug_struct("CreateChainResponse")
            .field("id", id)
            .field("claim_details", claim_details)
            .field("lockup_details", lockup_details)
            .field("swap_auth", &RedactedSwapAuth(swap_auth))
            .finish()
    }
}
impl CreateChainResponse {
    /// Validate chain swap response
    pub fn validate(
        &self,
        claim_pubkey: &PublicKey,
        refund_pubkey: &PublicKey,
        from_chain: Chain,
        to_chain: Chain,
    ) -> Result<(), Error> {
        self.validate_with_currency(
            claim_pubkey,
            refund_pubkey,
            from_chain,
            to_chain,
            None,
            None,
        )
    }

    pub fn validate_with_currency(
        &self,
        claim_pubkey: &PublicKey,
        refund_pubkey: &PublicKey,
        from_chain: Chain,
        to_chain: Chain,
        from_currency: Option<Currency>,
        to_currency: Option<Currency>,
    ) -> Result<(), Error> {
        self.validate_with_currency_and_asset_context(
            claim_pubkey,
            refund_pubkey,
            from_chain,
            to_chain,
            from_currency,
            to_currency,
            None,
            None,
        )
    }

    /// Validate both create-response legs against their requested currencies
    /// and the asset ids from the pair card the caller accepted.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_with_currency_and_asset_context(
        &self,
        claim_pubkey: &PublicKey,
        refund_pubkey: &PublicKey,
        from_chain: Chain,
        to_chain: Chain,
        from_currency: Option<Currency>,
        to_currency: Option<Currency>,
        from_asset_context: Option<LiquidAssetContext>,
        to_asset_context: Option<LiquidAssetContext>,
    ) -> Result<(), Error> {
        self.validate_side(
            Side::Lockup,
            from_chain,
            from_currency,
            from_asset_context,
            &self.lockup_details,
            refund_pubkey,
        )?;
        self.validate_side(
            Side::Claim,
            to_chain,
            to_currency,
            to_asset_context,
            &self.claim_details,
            claim_pubkey,
        )
    }

    fn validate_side(
        &self,
        side: Side,
        chain: Chain,
        currency: Option<Currency>,
        expected_asset_context: Option<LiquidAssetContext>,
        details: &ChainSwapDetails,
        our_pubkey: &PublicKey,
    ) -> Result<(), Error> {
        match chain {
            Chain::Bitcoin(bitcoin_chain) => {
                let boltz_chain_script =
                    BtcSwapScript::chain_from_swap_resp(side, details.clone(), *our_pubkey)?;
                boltz_chain_script.validate_address(bitcoin_chain, details.lockup_address.clone())
            }
            Chain::Liquid(liquid_chain) => {
                let boltz_chain_script =
                    LiquidSwapScript::chain_from_swap_resp(side, details.clone(), *our_pubkey)?;
                boltz_chain_script.validate_currency(
                    liquid_chain,
                    chain.resolve_currency(currency)?,
                    expected_asset_context,
                )?;
                boltz_chain_script.validate_address(liquid_chain, details.lockup_address.clone())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSwapTx {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSwapTxTimeout {
    pub block_height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSwapTxLock {
    pub transaction: ChainSwapTx,
    pub timeout: ChainSwapTxTimeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSwapTxResp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_lock: Option<ChainSwapTxLock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_lock: Option<ChainSwapTxLock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseSwapTxResp {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    pub timeout_block_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmarineSwapTxResp {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_block_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_eta: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmarineSwapPreimageResp {
    pub preimage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialSig {
    pub pub_nonce: String,
    pub partial_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToSign {
    pub pub_nonce: String,
    pub transaction: String,
    pub index: u32,
}

#[derive(Debug, Clone)]
pub struct Cooperative<'a> {
    pub boltz_api: &'a BoltzApiClientV2,
    pub swap_id: String,
    /// The signature (partial_sig + pub_nonce) is needed to post the claim tx details of the Chain swap
    /// It may be omitted for a chain swap if we've already sent the signature to Boltz
    pub signature: Option<(musig::PartialSignature, musig::PublicNonce)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapUpdateTxDetails {
    pub id: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespError {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwapTxKind {
    Claim,
    Refund,
}

/// States for a submarine swap.
///
/// See <https://docs.boltz.exchange/v/api/lifecycle#normal-submarine-swaps>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubSwapStates {
    /// Initial state of the swap; optionally the initial state can also be `invoice.set` in case
    /// the invoice was already specified in the request that created the swap.
    #[serde(rename = "swap.created")]
    Created,
    /// The lockup transaction was found in the mempool, meaning the user sent funds to the
    /// lockup address.
    #[serde(rename = "transaction.mempool")]
    TransactionMempool,
    /// The lockup transaction was included in a block.
    #[serde(rename = "transaction.confirmed")]
    TransactionConfirmed,
    /// The swap has an invoice that should be paid.
    /// Can be the initial state when the invoice was specified in the request that created the swap
    #[serde(rename = "invoice.set")]
    InvoiceSet,
    /// Boltz successfully paid the invoice.
    #[serde(rename = "invoice.paid")]
    InvoicePaid,
    /// Boltz started paying the invoice.
    #[serde(rename = "invoice.pending")]
    InvoicePending,
    /// Boltz failed to pay the invoice. In this case the user needs to broadcast a refund
    /// transaction to reclaim the locked up onchain coins.
    #[serde(rename = "invoice.failedToPay")]
    InvoiceFailedToPay,
    /// Indicates that after the invoice was successfully paid, the onchain were successfully
    /// claimed by Boltz. This is the final status of a successful Normal Submarine Swap.
    #[serde(rename = "transaction.claimed")]
    TransactionClaimed,
    /// Indicates that Boltz is ready for the creation of a cooperative signature for a key path
    /// spend. Taproot Swaps are not claimed immediately by Boltz after the invoice has been paid,
    /// but instead Boltz waits for the API client to post a signature for a key path spend. If the
    /// API client does not cooperate in a key path spend, Boltz will eventually claim via the script path.
    #[serde(rename = "transaction.claim.pending")]
    TransactionClaimPending,
    /// Indicates the lockup failed, which is usually because the user sent too little.
    #[serde(rename = "transaction.lockupFailed")]
    TransactionLockupFailed,
    /// Indicates the user didn't send onchain (lockup) and the swap expired (approximately 24h).
    /// This means that it was cancelled and chain L-BTC shouldn't be sent anymore.
    #[serde(rename = "swap.expired")]
    SwapExpired,
}

impl Display for SubSwapStates {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SubSwapStates::Created => "swap.created".to_string(),
            SubSwapStates::TransactionMempool => "transaction.mempool".to_string(),
            SubSwapStates::TransactionConfirmed => "transaction.confirmed".to_string(),
            SubSwapStates::InvoiceSet => "invoice.set".to_string(),
            SubSwapStates::InvoicePaid => "invoice.paid".to_string(),
            SubSwapStates::InvoicePending => "invoice.pending".to_string(),
            SubSwapStates::InvoiceFailedToPay => "invoice.failedToPay".to_string(),
            SubSwapStates::TransactionClaimed => "transaction.claimed".to_string(),
            SubSwapStates::TransactionClaimPending => "transaction.claim.pending".to_string(),
            SubSwapStates::TransactionLockupFailed => "transaction.lockupFailed".to_string(),
            SubSwapStates::SwapExpired => "swap.expired".to_string(),
        };
        write!(f, "{str}")
    }
}

impl FromStr for SubSwapStates {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "swap.created" => Ok(SubSwapStates::Created),
            "transaction.mempool" => Ok(SubSwapStates::TransactionMempool),
            "transaction.confirmed" => Ok(SubSwapStates::TransactionConfirmed),
            "invoice.set" => Ok(SubSwapStates::InvoiceSet),
            "invoice.paid" => Ok(SubSwapStates::InvoicePaid),
            "invoice.pending" => Ok(SubSwapStates::InvoicePending),
            "invoice.failedToPay" => Ok(SubSwapStates::InvoiceFailedToPay),
            "transaction.claimed" => Ok(SubSwapStates::TransactionClaimed),
            "transaction.claim.pending" => Ok(SubSwapStates::TransactionClaimPending),
            "transaction.lockupFailed" => Ok(SubSwapStates::TransactionLockupFailed),
            "swap.expired" => Ok(SubSwapStates::SwapExpired),
            _ => Err(()),
        }
    }
}

/// States for a reverse swap.
///
/// See <https://docs.boltz.exchange/v/api/lifecycle#reverse-submarine-swaps>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RevSwapStates {
    /// Initial state of a newly created Reverse Submarine Swap.
    #[serde(rename = "swap.created")]
    Created,
    /// Optional and currently not enabled on Boltz. If Boltz requires prepaying miner fees via a
    /// separate Lightning invoice, this state is set when the miner fee invoice was successfully paid.
    #[serde(rename = "minerfee.paid")]
    MinerFeePaid,
    /// Boltz's lockup transaction is found in the mempool which will only happen after the user
    /// paid the Lightning hold invoice.
    #[serde(rename = "transaction.mempool")]
    TransactionMempool,
    /// The lockup transaction was included in a block. This state is skipped, if the client
    /// optionally accepts the transaction without confirmation. Boltz broadcasts chain transactions
    /// non-RBF only.
    #[serde(rename = "transaction.confirmed")]
    TransactionConfirmed,
    /// The transaction claiming onchain was broadcast by the user's client and Boltz used the
    /// preimage of this transaction to settle the Lightning invoice. This is the final status of a
    /// successful Reverse Submarine Swap.
    #[serde(rename = "invoice.settled")]
    InvoiceSettled,
    /// Set when the invoice of Boltz expired and pending HTLCs are cancelled. Boltz invoices
    /// currently expire after 50% of the swap timeout window.
    #[serde(rename = "invoice.expired")]
    InvoiceExpired,
    /// This is the final status of a swap, if the swap expires without the lightning invoice being paid.
    #[serde(rename = "swap.expired")]
    SwapExpired,
    /// Set in the unlikely event that Boltz is unable to send the agreed amount of onchain coins
    /// after the user set up the payment to the provided Lightning invoice. If this happens, the
    /// pending Lightning HTLC will also be cancelled. The Lightning bitcoin automatically bounce
    /// back to the user, no further action or refund is required and the user didn't pay any fees.
    #[serde(rename = "transaction.failed")]
    TransactionFailed,
    /// This is the final status of a swap, if the user successfully set up the Lightning payment
    /// and Boltz successfully locked up coins onchain, but the Boltz API Client did not claim
    /// the locked oncahin coins before swap expiry. In this case, Boltz will also automatically refund
    /// its own locked onchain coins and the Lightning payment is cancelled.
    #[serde(rename = "transaction.refunded")]
    TransactionRefunded,
}

impl Display for RevSwapStates {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RevSwapStates::Created => "swap.created".to_string(),
            RevSwapStates::MinerFeePaid => "minerfee.paid".to_string(),
            RevSwapStates::TransactionMempool => "transaction.mempool".to_string(),
            RevSwapStates::TransactionConfirmed => "transaction.confirmed".to_string(),
            RevSwapStates::InvoiceSettled => "invoice.settled".to_string(),
            RevSwapStates::InvoiceExpired => "invoice.expired".to_string(),
            RevSwapStates::SwapExpired => "swap.expired".to_string(),
            RevSwapStates::TransactionFailed => "transaction.failed".to_string(),
            RevSwapStates::TransactionRefunded => "transaction.refunded".to_string(),
        };
        write!(f, "{str}")
    }
}

impl FromStr for RevSwapStates {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "swap.created" => Ok(RevSwapStates::Created),
            "minerfee.paid" => Ok(RevSwapStates::MinerFeePaid),
            "transaction.mempool" => Ok(RevSwapStates::TransactionMempool),
            "transaction.confirmed" => Ok(RevSwapStates::TransactionConfirmed),
            "invoice.settled" => Ok(RevSwapStates::InvoiceSettled),
            "invoice.expired" => Ok(RevSwapStates::InvoiceExpired),
            "swap.expired" => Ok(RevSwapStates::SwapExpired),
            "transaction.failed" => Ok(RevSwapStates::TransactionFailed),
            "transaction.refunded" => Ok(RevSwapStates::TransactionRefunded),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChainSwapStates {
    /// The initial state of the chain swap.
    #[serde(rename = "swap.created")]
    Created,
    /// The server has rejected a 0-conf transaction for this swap.
    #[serde(rename = "transaction.zeroconf.rejected")]
    TransactionZeroConfRejected,
    /// The lockup transaction of the client was found in the mempool.
    #[serde(rename = "transaction.mempool")]
    TransactionMempool,
    /// The lockup transaction of the client was confirmed in a block. When the server accepts 0-conf,
    /// for the lockup transaction, this state is skipped.
    #[serde(rename = "transaction.confirmed")]
    TransactionConfirmed,
    /// The lockup transaction of the server has been broadcast.
    #[serde(rename = "transaction.server.mempool")]
    TransactionServerMempool,
    /// The lockup transaction of the server has been included in a block.
    #[serde(rename = "transaction.server.confirmed")]
    TransactionServerConfirmed,
    /// The server claimed the coins that the client locked.
    #[serde(rename = "transaction.claimed")]
    TransactionClaimed,
    /// Indicates the lockup failed, which is usually because the user sent too little.
    #[serde(rename = "transaction.lockupFailed")]
    TransactionLockupFailed,
    /// This is the final status of a swap, if the swap expires without a chain bitcoin transaction.
    #[serde(rename = "swap.expired")]
    SwapExpired,
    /// Set in the unlikely event that Boltz is unable to lock the agreed amount of chain bitcoin.
    /// The user needs to submit a refund transaction to reclaim the chain bitcoin if bitcoin were
    /// already sent.
    #[serde(rename = "transaction.failed")]
    TransactionFailed,
    /// If the user and Boltz both successfully locked up bitcoin on the chain, but the user did not
    /// claim the locked chain bitcoin until swap expiry, Boltz will automatically refund its own locked
    /// chain bitcoin.
    #[serde(rename = "transaction.refunded")]
    TransactionRefunded,
}

impl Display for ChainSwapStates {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            ChainSwapStates::Created => "swap.created".to_string(),
            ChainSwapStates::TransactionZeroConfRejected => {
                "transaction.zeroconf.rejected".to_string()
            }
            ChainSwapStates::TransactionMempool => "transaction.mempool".to_string(),
            ChainSwapStates::TransactionConfirmed => "transaction.confirmed".to_string(),
            ChainSwapStates::TransactionServerMempool => "transaction.server.mempool".to_string(),
            ChainSwapStates::TransactionServerConfirmed => {
                "transaction.server.confirmed".to_string()
            }
            ChainSwapStates::TransactionClaimed => "transaction.claimed".to_string(),
            ChainSwapStates::TransactionLockupFailed => "transaction.lockupFailed".to_string(),
            ChainSwapStates::SwapExpired => "swap.expired".to_string(),
            ChainSwapStates::TransactionFailed => "transaction.failed".to_string(),
            ChainSwapStates::TransactionRefunded => "transaction.refunded".to_string(),
        };
        write!(f, "{str}")
    }
}

impl FromStr for ChainSwapStates {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "swap.created" => Ok(ChainSwapStates::Created),
            "transaction.zeroconf.rejected" => Ok(ChainSwapStates::TransactionZeroConfRejected),
            "transaction.mempool" => Ok(ChainSwapStates::TransactionMempool),
            "transaction.confirmed" => Ok(ChainSwapStates::TransactionConfirmed),
            "transaction.server.mempool" => Ok(ChainSwapStates::TransactionServerMempool),
            "transaction.server.confirmed" => Ok(ChainSwapStates::TransactionServerConfirmed),
            "transaction.claimed" => Ok(ChainSwapStates::TransactionClaimed),
            "transaction.lockupFailed" => Ok(ChainSwapStates::TransactionLockupFailed),
            "swap.expired" => Ok(ChainSwapStates::SwapExpired),
            "transaction.failed" => Ok(ChainSwapStates::TransactionFailed),
            "transaction.refunded" => Ok(ChainSwapStates::TransactionRefunded),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum SwapType {
    Submarine,
    ReverseSubmarine,
    Chain,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl Display for OrderSide {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        f.write_str(str)
    }
}

impl FromStr for OrderSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "buy" => Ok(OrderSide::Buy),
            "sell" => Ok(OrderSide::Sell),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetFeeEstimationResponse {
    #[serde(rename = "BTC")]
    pub btc: f64,
    #[serde(rename = "L-BTC")]
    pub lbtc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBolt12OfferRequest {
    pub offer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBolt12OfferRequest {
    pub offer: String,
    /// The updated webhook URL.
    /// Setting to None will remove the webhook URL from the registered Offer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The schnorr signature of the SHA256 hash of the webhook URL or "UPDATE" when None
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagicRoutingHint {
    pub bip21: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBolt12FetchRequest {
    /// The offer to fetch an invoice for
    pub offer: String,
    /// The amount to pay, in satoshi
    pub amount: u64,
    /// The optional payer note
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBolt12FetchResponse {
    /// BOLT12 invoice
    pub invoice: String,
    /// The invoice magic routing hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_routing_hint: Option<MagicRoutingHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBolt12ParamsResponse {
    /// Minimum CLTV value
    pub min_cltv: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// The public key
    pub public_key: secp256k1::PublicKey,
    /// The public URIs
    pub uris: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodesResponse {
    #[serde(rename = "BTC")]
    pub btc: HashMap<String, Node>,
}

impl GetNodesResponse {
    /// Get the BTC LND node data from the response.
    /// Returns None if not found.
    pub fn get_btc_lnd_node(&self) -> Option<Node> {
        self.btc.get("LND").cloned()
    }

    /// Get the BTC CLN node data from the response.
    /// Returns None if not found.
    pub fn get_btc_cln_node(&self) -> Option<Node> {
        self.btc.get("CLN").cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetQuoteResponse {
    /// Server lockup amount, in sat
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub id: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionOut {
    pub id: String,
    pub vout: u32,
}

/// One entry of a swap's history, as served in [`GetSwapResponse::events`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SwapEvent {
    /// What happened, e.g. `invoice_issued`, `expired`.
    pub kind: String,
    /// Unix seconds.
    pub ts: i64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetSwapResponse {
    pub status: String,
    pub zero_conf_rejected: Option<bool>,
    pub transaction: Option<TransactionResponse>,
    /// Everything below is served by the KaleidoSwap maker and absent from
    /// Boltz, so each one is optional: an unmodelled field is dropped by
    /// serde, and callers were getting back `status` alone with no way to see
    /// what a swap had done or why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// `submarine` / `reverse` / `chain`. Named around the `type` keyword.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub swap_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_details: Option<String>,
    /// The swap's history, oldest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<SwapEvent>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    /// Every endpoint `default()` hands out must be a KaleidoSwap maker, and it
    /// must be on the same chain as that network's default chain access — see
    /// `signet_resolves_to_signet_not_testnet3`. Networks we run no maker on
    /// error out; a third-party fallback would hand the caller a counterparty
    /// they never chose.
    /// A swap status must carry the history and failure detail the maker
    /// sends, not just `status`.
    ///
    /// The unmodelled fields were dropped by serde, so callers saw
    /// `{"status": ...}` alone — no `events` to tell what a swap had done, and
    /// no `failureReason` to say why it stopped.
    #[test]
    fn swap_status_keeps_the_maker_history_and_failure_fields() {
        // Exactly what the maker serves for a created reverse swap.
        let body = serde_json::json!({
            "events": [{"kind": "invoice_issued", "ts": 1_786_704_659_i64}],
            "failureDetails": null,
            "failureReason": null,
            "id": "01KZZYB138E7C3HZX7Q1YBGAQG",
            "paymentStatus": "pending",
            "status": "swap.created",
            "type": "reverse",
        });

        let parsed: GetSwapResponse = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.status, "swap.created");
        assert_eq!(parsed.id.as_deref(), Some("01KZZYB138E7C3HZX7Q1YBGAQG"));
        assert_eq!(parsed.swap_type.as_deref(), Some("reverse"));
        assert_eq!(parsed.payment_status.as_deref(), Some("pending"));
        assert_eq!(
            parsed.events.as_deref(),
            Some(
                [SwapEvent {
                    kind: "invoice_issued".to_owned(),
                    ts: 1_786_704_659,
                }]
                .as_slice()
            ),
        );

        // Round-trips back out under the wire names the maker uses.
        let out = serde_json::to_value(&parsed).unwrap();
        assert_eq!(out["type"], "reverse");
        assert_eq!(out["paymentStatus"], "pending");
        assert_eq!(out["events"][0]["kind"], "invoice_issued");

        // A Boltz maker sends none of them; that must still parse, and must
        // not invent keys on the way back out.
        let boltz: GetSwapResponse =
            serde_json::from_value(serde_json::json!({ "status": "invoice.set" })).unwrap();
        assert_eq!(boltz.status, "invoice.set");
        assert!(boltz.events.is_none());
        let out = serde_json::to_value(&boltz).unwrap();
        assert!(out.get("events").is_none());
        assert!(out.get("type").is_none());
    }

    /// All three create requests must put the caller's rate lock on the wire.
    ///
    /// `pair_hash` was absent from `CreateReverseRequest` while submarine and
    /// chain both carried it, so the maker had nothing to check a reverse swap
    /// against and created it at whatever the rate happened to be — silently,
    /// on exactly one of the three routes.
    #[test]
    fn every_create_request_serialises_pair_hash() {
        let hash = "20461469e74be40d9faa21ff9d1f654ab30387fbacb9a6e1c9473fab4d7e5f3c";
        let key: PublicKey = "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2"
            .parse()
            .unwrap();

        let reverse = serde_json::to_value(CreateReverseRequest {
            from: "BTC".to_owned(),
            to: "L-BTC".to_owned(),
            claim_public_key: key,
            invoice: None,
            invoice_amount: Some(100_000),
            preimage_hash: None,
            pair_hash: Some(hash.to_owned()),
            description: None,
            description_hash: None,
            address: None,
            address_signature: None,
            referral_id: None,
            webhook: None,
        })
        .unwrap();
        assert_eq!(
            reverse["pairHash"], hash,
            "reverse must send the rate lock: {reverse}"
        );

        let submarine = serde_json::to_value(CreateSubmarineRequest {
            from: "L-USDT".to_owned(),
            to: "BTC".to_owned(),
            invoice: "lnbcrt1".to_owned(),
            refund_public_key: key,
            pair_hash: Some(hash.to_owned()),
            referral_id: None,
            webhook: None,
        })
        .unwrap();
        assert_eq!(submarine["pairHash"], hash);

        // Omitted stays omitted — the field is optional, not defaulted.
        let without = serde_json::to_value(CreateReverseRequest {
            from: "BTC".to_owned(),
            to: "L-BTC".to_owned(),
            claim_public_key: key,
            invoice: None,
            invoice_amount: Some(100_000),
            preimage_hash: None,
            pair_hash: None,
            description: None,
            description_hash: None,
            address: None,
            address_signature: None,
            referral_id: None,
            webhook: None,
        })
        .unwrap();
        assert!(without.get("pairHash").is_none());
    }

    /// A chain create response as the KaleidoSwap maker sends it, with or
    /// without the credential.
    fn chain_create_response_json(swap_auth: Option<&str>) -> serde_json::Value {
        let mut value = serde_json::json!({
            "id": "01KZZYB138E7C3HZX7Q1YBGAQG",
            "claimDetails": {
                "swapTree": {
                    "claimLeaf": {"output": "00", "version": 196},
                    "refundLeaf": {"output": "01", "version": 196},
                },
                "lockupAddress": "bcrt1qclaim",
                "serverPublicKey":
                    "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
                "timeoutBlockHeight": 200,
                "amount": 100_000,
            },
            "lockupDetails": {
                "swapTree": {
                    "claimLeaf": {"output": "02", "version": 196},
                    "refundLeaf": {"output": "03", "version": 196},
                },
                "lockupAddress": "bcrt1qlockup",
                "serverPublicKey":
                    "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
                "timeoutBlockHeight": 300,
                "amount": 100_000,
            },
        });
        if let Some(swap_auth) = swap_auth {
            value["swapAuth"] = serde_json::json!(swap_auth);
        }
        value
    }

    /// Every create response must keep the maker's per-swap taker credential.
    ///
    /// The three response structs did not model `swapAuth`, so serde dropped
    /// it. It is issued exactly once, nothing re-issues it, and it is the only
    /// thing that can accept a chain re-quote — so dropping it left the swap
    /// with no path but its refund.
    #[test]
    fn create_responses_keep_the_swap_auth_credential() {
        let auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";

        let chain: CreateChainResponse =
            serde_json::from_value(chain_create_response_json(Some(auth))).unwrap();
        assert_eq!(chain.swap_auth.as_deref(), Some(auth));
        // Round-trips out under the wire name, so a caller that persists the
        // response as JSON keeps the credential.
        assert_eq!(serde_json::to_value(&chain).unwrap()["swapAuth"], auth);

        let reverse: CreateReverseResponse = serde_json::from_value(serde_json::json!({
            "id": "01KZZYB138E7C3HZX7Q1YBGAQG",
            "invoice": "lnbcrt1",
            "swapTree": {
                "claimLeaf": {"output": "00", "version": 196},
                "refundLeaf": {"output": "01", "version": 196},
            },
            "lockupAddress": "bcrt1qlockup",
            "refundPublicKey":
                "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
            "timeoutBlockHeight": 200,
            "onchainAmount": 100_000,
            "swapAuth": auth,
        }))
        .unwrap();
        assert_eq!(reverse.swap_auth.as_deref(), Some(auth));

        let submarine: CreateSubmarineResponse = serde_json::from_value(serde_json::json!({
            "acceptZeroConf": false,
            "address": "bcrt1qlockup",
            "bip21": "bitcoin:bcrt1qlockup?amount=0.001",
            "claimPublicKey":
                "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
            "expectedAmount": 100_000,
            "id": "01KZZYB138E7C3HZX7Q1YBGAQG",
            "swapTree": {
                "claimLeaf": {"output": "00", "version": 196},
                "refundLeaf": {"output": "01", "version": 196},
            },
            "timeoutBlockHeight": 200,
            "swapAuth": auth,
        }))
        .unwrap();
        assert_eq!(submarine.swap_auth.as_deref(), Some(auth));

        // A maker that issues none — upstream Boltz declares no auth on the
        // accept route — must still parse, and must not gain a null key on the
        // way back out.
        let boltz: CreateReverseResponse = serde_json::from_value(serde_json::json!({
            "id": "01KZZYB138E7C3HZX7Q1YBGAQG",
            "invoice": "lnbcrt1",
            "swapTree": {
                "claimLeaf": {"output": "00", "version": 196},
                "refundLeaf": {"output": "01", "version": 196},
            },
            "lockupAddress": "bcrt1qlockup",
            "refundPublicKey":
                "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
            "timeoutBlockHeight": 200,
            "onchainAmount": 100_000,
        }))
        .unwrap();
        assert!(boltz.swap_auth.is_none());
        assert!(serde_json::to_value(&boltz)
            .unwrap()
            .get("swapAuth")
            .is_none());
    }

    /// The credential must reach the maker in the header it reads, and only
    /// when the caller has one.
    ///
    /// A request that silently went out without it would come back
    /// `401 invalid_swap_auth`, which is indistinguishable from a wrong
    /// credential.
    #[test]
    fn swap_auth_travels_in_the_header_the_maker_reads() {
        let client = BoltzApiClientV2::new(BOLTZ_REGTEST.to_string(), None);
        let url = format!("{BOLTZ_REGTEST}/swap/chain/some-id/quote");
        let auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";

        // Pinned to the literal the maker matches on, not to the constant the
        // code writes with: reading the name back out of `SWAP_AUTH_HEADER`
        // would pass under any rename, and a renamed header is exactly the
        // `401` this whole change exists to avoid.
        assert_eq!(SWAP_AUTH_HEADER, "X-Swap-Auth");

        let with_auth =
            BoltzApiClientV2::maybe_add_swap_auth(client.http_client.post(&url), Some(auth))
                .unwrap()
                .build()
                .unwrap();
        assert_eq!(
            with_auth.headers().get("X-Swap-Auth").unwrap(),
            auth,
            "the credential must go out in X-Swap-Auth",
        );

        // No credential means no header at all, not an empty one: an empty
        // value is a *wrong* credential to the maker, not an absent one.
        let without = BoltzApiClientV2::maybe_add_swap_auth(client.http_client.post(&url), None)
            .unwrap()
            .build()
            .unwrap();
        assert!(without.headers().get("X-Swap-Auth").is_none());

        // A value that cannot be a header fails here, naming the credential,
        // rather than as a bare send failure that reads like the maker refused
        // the swap.
        let err = BoltzApiClientV2::maybe_add_swap_auth(
            client.http_client.post(&url),
            Some("bad\nvalue"),
        )
        .unwrap_err();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains(SWAP_AUTH_HEADER)),
            "expected a named credential error, got {err:?}",
        );

        // An empty credential is a well-formed header the maker can only
        // answer `401`, which reads as "wrong credential" — so it fails here
        // instead, saying it is empty.
        let err = BoltzApiClientV2::maybe_add_swap_auth(client.http_client.post(&url), Some(""))
            .unwrap_err();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains("empty")),
            "expected an empty-credential error, got {err:?}",
        );
    }

    /// A single-request stand-in for the maker: binds an ephemeral port, hands
    /// back the base URL to point a client at, and yields the raw request it
    /// received.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn capture_one_request() -> (String, std::thread::JoinHandle<String>) {
        capture_one_request_answering(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        )
    }

    /// [`capture_one_request`] with a response of the test's choosing.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn capture_one_request_answering(
        response: Vec<u8>,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v2", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let read = stream.read(&mut buf).unwrap();
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..read]);
                let Some(head_end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                // Drain the body as well before answering. Closing the socket
                // under bytes the client is still writing reaches `reqwest` as
                // a connection reset rather than as the 200 below.
                let body_len: usize = String::from_utf8_lossy(&raw[..head_end])
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())?
                    })
                    .unwrap_or(0);
                if raw.len() >= head_end + 4 + body_len {
                    break;
                }
            }
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
            String::from_utf8_lossy(&raw).into_owned()
        });
        (base_url, handle)
    }

    /// The credential must survive the trip from `accept_quote` to the wire.
    ///
    /// `swap_auth_travels_in_the_header_the_maker_reads` pins the header helper
    /// in isolation, which leaves the threading between them untested: route
    /// `accept_quote` back through `post_json`, or drop the argument in a
    /// refactor, and the credential silently stops being sent while both other
    /// tests stay green. Every accept then comes back `401 invalid_swap_auth`
    /// and the swap runs out to its refund path.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn accept_quote_sends_the_credential_to_the_maker() {
        let auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";
        let header = SWAP_AUTH_HEADER.to_lowercase();

        let (base_url, maker) = capture_one_request();
        BoltzApiClientV2::new(base_url, None)
            .accept_quote("01KZZYB138E7C3HZX7Q1YBGAQG", 100_000, Some(auth))
            .await
            .unwrap();
        let request = maker.join().unwrap().to_lowercase();

        assert!(
            request.starts_with("post /v2/swap/chain/01kzzyb138e7c3hzx7q1ybgaqg/quote "),
            "wrong route, got:\n{request}",
        );
        assert!(
            request.contains(&format!("{header}: {auth}")),
            "accept_quote must put the credential on the wire, got:\n{request}",
        );

        // ...and must send no header at all for a maker that issues none, which
        // is a different thing to the maker than an empty one.
        let (base_url, maker) = capture_one_request();
        BoltzApiClientV2::new(base_url, None)
            .accept_quote("01KZZYB138E7C3HZX7Q1YBGAQG", 100_000, None)
            .await
            .unwrap();
        let request = maker.join().unwrap().to_lowercase();

        assert!(
            !request.contains(&header),
            "no credential means no header, got:\n{request}",
        );
    }

    /// A redirect that carried the credential elsewhere must be reported.
    ///
    /// `reqwest` drops only `Authorization`, `Cookie` and `Proxy-Authorization`
    /// when a redirect crosses origins, so a custom header follows a `302` to
    /// whatever host the `Location` names — this test pins that behaviour as
    /// much as it pins the reaction to it. Clients from
    /// [`BoltzApiClientV2::default_http_client`] follow no redirects and so
    /// never get here; a caller-supplied client, or the browser, can. Nothing
    /// can call the credential back at that point, so the requirement is that
    /// the caller be told rather than handed a parsed response from a host they
    /// never addressed.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn a_redirect_that_carried_the_credential_is_reported() {
        let auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";

        let (elsewhere_url, elsewhere) = capture_one_request();
        let elsewhere_host = elsewhere_url
            .strip_prefix("http://")
            .and_then(|url| url.strip_suffix("/v2"))
            .unwrap()
            .to_string();
        let (base_url, maker) = capture_one_request_answering(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{elsewhere_host}/v2/elsewhere\r\n\
                 Content-Length: 0\r\n\r\n"
            )
            .into_bytes(),
        );

        // A caller-supplied client on reqwest's default policy, which follows.
        let follows_redirects = reqwest::Client::builder().build().unwrap();
        let err = BoltzApiClientV2::with_client(base_url, follows_redirects, None)
            .accept_quote("01KZZYB138E7C3HZX7Q1YBGAQG", 100_000, Some(auth))
            .await
            .unwrap_err();

        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains(SWAP_AUTH_HEADER)),
            "expected the disclosure to be named, got {err:?}",
        );

        maker.join().unwrap();
        let forwarded = elsewhere.join().unwrap().to_lowercase();
        assert!(
            forwarded.contains(&format!("{}: {auth}", SWAP_AUTH_HEADER.to_lowercase())),
            "reqwest forwards the credential across the hop \u{2014} that is what the \
             error is reporting, and what default_http_client exists to prevent. \
             Got:\n{forwarded}",
        );
    }

    /// A client built for a partner organization must put the key in
    /// `Authorization: Bearer …`, and one built without must send no such header
    /// at all.
    ///
    /// The header name is pinned to the literal the maker reads rather than to
    /// the constant the code writes with — reading it back out of
    /// [`API_KEY_HEADER`] would pass under any rename, and a renamed header is a
    /// swap the maker records as anonymous while the partner waits for it to
    /// appear in their statistics.
    #[test]
    fn the_organization_key_travels_in_the_authorization_header() {
        assert_eq!(API_KEY_HEADER, "Authorization");

        let key = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz";
        let base_url = "https://maker.signet.kaleidoswap.com/v2";
        let url = format!("{base_url}/swap/submarine");

        let authenticated = BoltzApiClientV2::new(base_url.to_string(), None)
            .with_api_key(key.parse::<crate::kaleido::ApiKey>().unwrap());
        let request = authenticated
            .maybe_add_api_key(authenticated.http_client.post(&url), &url)
            .unwrap()
            .build()
            .unwrap();
        let sent = request.headers().get("Authorization").unwrap();
        assert_eq!(sent.to_str().unwrap(), format!("Bearer {key}"));
        assert!(
            sent.is_sensitive(),
            "the key must be marked sensitive, or HTTP/2 indexes it into the HPACK \
             dynamic table",
        );

        // The generic client authenticates nothing — that is the whole contract
        // with a Boltz maker, which has no notion of an organization key.
        let generic = BoltzApiClientV2::new(base_url.to_string(), None);
        let request = generic
            .maybe_add_api_key(generic.http_client.post(&url), &url)
            .unwrap()
            .build()
            .unwrap();
        assert!(request.headers().get("Authorization").is_none());
    }

    /// The key must never be attached to a request addressed anywhere but the
    /// maker it was configured for.
    ///
    /// It is a permanent organization credential. Esplora, the Platform API, a
    /// merchant webhook and a second maker would each learn a secret they have
    /// no use for, and any of them could then attribute their own swaps to that
    /// organization. Nothing in this client builds such a URL today; the check
    /// is what keeps that true when something does.
    #[test]
    fn the_organization_key_is_bound_to_the_configured_origin() {
        let base_url = "https://maker.signet.kaleidoswap.com/v2";
        let client = BoltzApiClientV2::new(base_url.to_string(), None).with_api_key(
            "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t"
                .parse::<crate::kaleido::ApiKey>()
                .unwrap(),
        );

        for elsewhere in [
            "https://blockstream.info/api/tx",
            "https://api.kaleidoswap.com/v1/whatever",
            "https://maker.signet.kaleidoswap.evil/v2/swap/submarine",
            // Same host, different port: a different server.
            "https://maker.signet.kaleidoswap.com:8443/v2/swap/submarine",
            // Same host, plain HTTP: the key would go out in clear.
            "http://maker.signet.kaleidoswap.com/v2/swap/submarine",
        ] {
            let err = client
                .maybe_add_api_key(client.http_client.post(elsewhere), elsewhere)
                .unwrap_err();
            assert!(
                matches!(&err, Error::Protocol(msg) if msg.contains("refusing to send")),
                "the key must not go to {elsewhere}, got {err:?}",
            );
            // ...and the message must not be the thing that leaks it.
            assert!(!format!("{err:?}").contains("s3cr3t"), "{err:?}");
        }

        // Any path under the configured origin is the maker.
        let same = format!("{base_url}/swap/chain/some-id/quote");
        assert!(client
            .maybe_add_api_key(client.http_client.post(&same), &same)
            .is_ok());
    }

    /// The key must reach the wire on every route, not only on create.
    ///
    /// Attribution is decided when the maker sees the request, and the SDK sends
    /// the key by carrying it on the client rather than by threading it through
    /// each call — so the test that matters is that a route nobody thought about
    /// still carries it.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn an_authenticated_client_sends_the_key_on_every_route() {
        let key = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz";
        let expected = format!("authorization: bearer {}", key.to_lowercase());

        // A GET route: no request body, nothing swap-specific, still attributed.
        let (base_url, maker) = capture_one_request();
        let client = BoltzApiClientV2::new(base_url, None)
            .with_api_key(key.parse::<crate::kaleido::ApiKey>().unwrap());
        let _ = client.get_height().await;
        let request = maker.join().unwrap().to_lowercase();
        assert!(
            request.starts_with("get /v2/chain/heights "),
            "wrong route, got:\n{request}",
        );
        assert!(
            request.contains(&expected),
            "a GET must carry the organization key, got:\n{request}",
        );

        // A POST route, alongside the per-swap credential: the two are separate
        // headers answering separate questions, and both must go out.
        let (base_url, maker) = capture_one_request();
        let client = BoltzApiClientV2::new(base_url, None)
            .with_api_key(key.parse::<crate::kaleido::ApiKey>().unwrap());
        let swap_auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";
        client
            .accept_quote("01KZZYB138E7C3HZX7Q1YBGAQG", 100_000, Some(swap_auth))
            .await
            .unwrap();
        let request = maker.join().unwrap().to_lowercase();
        assert!(
            request.contains(&expected),
            "a POST must carry the organization key, got:\n{request}",
        );
        assert!(
            request.contains(&format!("{}: {swap_auth}", SWAP_AUTH_HEADER.to_lowercase())),
            "the per-swap credential must still go out beside it, got:\n{request}",
        );

        // And an unauthenticated client sends neither.
        let (base_url, maker) = capture_one_request();
        let _ = BoltzApiClientV2::new(base_url, None).get_height().await;
        let request = maker.join().unwrap().to_lowercase();
        assert!(
            !request.contains("authorization:"),
            "the generic client must authenticate nothing, got:\n{request}",
        );
    }

    /// A response that came back from a host the client never addressed must
    /// fail, even though the key itself did not travel there.
    ///
    /// `reqwest` and `fetch` both drop `Authorization` across an origin hop, so
    /// the key is not disclosed — but the swap this would otherwise parse came
    /// from whoever answered, not from the maker, and the error has to say which
    /// of those two things happened so the caller knows whether to revoke
    /// anything.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn a_redirect_away_from_the_maker_fails_an_authenticated_request() {
        let key = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz";

        let (elsewhere_url, elsewhere) = capture_one_request();
        let elsewhere_host = elsewhere_url
            .strip_prefix("http://")
            .and_then(|url| url.strip_suffix("/v2"))
            .unwrap()
            .to_string();
        let (base_url, maker) = capture_one_request_answering(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{elsewhere_host}/v2/elsewhere\r\n\
                 Content-Length: 0\r\n\r\n"
            )
            .into_bytes(),
        );

        // A caller-supplied client on reqwest's default policy, which follows.
        let follows_redirects = reqwest::Client::builder().build().unwrap();
        let err = BoltzApiClientV2::with_client(base_url, follows_redirects, None)
            .with_api_key(key.parse::<crate::kaleido::ApiKey>().unwrap())
            .get_height()
            .await
            .unwrap_err();

        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains(API_KEY_HEADER)),
            "expected the hop to be reported, got {err:?}",
        );
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains("did not travel")),
            "the error must say the key stayed put, or the caller revokes a key \
             that was never exposed: {err:?}",
        );
        // The port is part of naming where it went: both ends are 127.0.0.1
        // here, and "redirected to 127.0.0.1" would read like a bug in the SDK.
        let redirected_to = elsewhere_url.strip_suffix("/v2").unwrap();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains(redirected_to)),
            "the error must name the origin including its port, got {err:?}",
        );

        maker.join().unwrap();
        let forwarded = elsewhere.join().unwrap().to_lowercase();
        assert!(
            !forwarded.contains("authorization:"),
            "reqwest must drop the key across the hop \u{2014} that is what the error \
             reports. Got:\n{forwarded}",
        );
    }

    /// A client built through `with_client_builder` must not follow a redirect,
    /// whatever the caller configured.
    ///
    /// This is the guarantee that makes the after-the-fact check unnecessary on
    /// native rather than load-bearing, and it cannot be recovered later: a
    /// `reqwest::Client` does not report its redirect policy, and a
    /// `reqwest::Response` carries only the URL the chain ended at — so a chain
    /// that detoured through another host and came back looks exactly like no
    /// redirect at all, while `X-Swap-Auth` rode along to the detour. Taking the
    /// builder instead of the built client is what closes that.
    ///
    /// `Location` points at a port nothing listens on, so following it could
    /// only fail as a connection error. Coming back as the `302` itself is the
    /// proof it was declined.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn a_client_from_a_caller_supplied_builder_declines_redirects() {
        use crate::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};

        let dead_port = {
            let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            socket.local_addr().unwrap().port()
        };
        let (base_url, maker) = capture_one_request_answering(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{dead_port}/v2/elsewhere\r\n\
                 Content-Length: 0\r\n\r\n"
            )
            .into_bytes(),
        );

        // `reqwest::Client::builder()` defaults to following up to 10 redirects.
        // The point is that the caller does not get to keep that here.
        let client = KaleidoMakerClient::with_client_builder(
            KaleidoMakerClientOptions {
                maker_url: base_url,
                api_key: "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t"
                    .parse::<ApiKey>()
                    .unwrap(),
                timeout: None,
            },
            reqwest::Client::builder(),
        )
        .expect("a loopback http maker is the regtest harness");

        let err = client.get_height().await.unwrap_err();
        assert!(
            matches!(&err, Error::HTTPStatusNotSuccess(status, _) if status.as_u16() == 302),
            "the 302 must surface as its own status, not be chased: {err:?}",
        );
        assert!(
            !matches!(&err, Error::Protocol(msg) if msg.contains("was redirected to")),
            "the hop must be declined outright, not reported after the fact: {err:?}",
        );

        maker.join().unwrap();
    }

    /// A redirect that changes only the scheme must say **revoke the key**.
    ///
    /// This is the one hop where the SDK's notion of an origin and `reqwest`'s
    /// disagree, and the disagreement runs the dangerous way. `same_origin` is
    /// scheme-sensitive; `redirect::remove_sensitive_headers` compares host and
    /// effective port and never looks at the scheme, so `https://h` →
    /// `http://h:443` keeps `Authorization` and re-sends a permanent
    /// organization credential in cleartext. Choosing the advice from the
    /// SDK's own rule would tell the partner there was nothing to revoke at
    /// exactly the moment their key hit the wire in the clear.
    ///
    /// Pinned against a constructed pair of URLs rather than a live socket:
    /// standing up an HTTPS listener with a trusted certificate for this is a
    /// lot of machinery to pin one boolean, and the boolean is the whole bug.
    #[test]
    fn a_scheme_only_redirect_says_revoke_the_key() {
        let secure: reqwest::Url = "https://maker.signet.kaleidoswap.com/v2".parse().unwrap();

        // Explicit :443 is how reqwest sees it: same host, same effective port,
        // so it keeps the header. The SDK still calls it a different origin.
        let downgraded: reqwest::Url = "http://maker.signet.kaleidoswap.com:443/v2/x"
            .parse()
            .unwrap();
        assert!(
            !same_origin(&secure, &downgraded),
            "a scheme change is a different origin to the SDK",
        );
        assert!(
            !redirect_strips_api_key(&secure, &downgraded),
            "reqwest compares host and port only — if this ever changes, the \
             advice below can be relaxed",
        );
        let advice = api_key_redirect_advice(&secure, &downgraded);
        assert!(advice.contains("revoke it"), "{advice}");
        assert!(
            !advice.contains("did not travel"),
            "the key did travel, in the clear: {advice}",
        );

        // A host change is the ordinary case, and there the *direct* hop really
        // does drop the header — so the partner must not be sent to revoke a
        // live key.
        let elsewhere: reqwest::Url = "https://elsewhere.example.com/v2/x".parse().unwrap();
        assert!(redirect_strips_api_key(&secure, &elsewhere));
        let advice = api_key_redirect_advice(&secure, &elsewhere);
        assert!(advice.contains("did not travel"), "{advice}");
        assert!(!advice.contains("revoke it"), "{advice}");
        // ...but "direct" is all this function can see. `reqwest` applies its
        // rule to each hop against the one before it, while a `Response` carries
        // only the URL the chain ended at, so `https://maker` →
        // `http://maker:443` → `https://elsewhere` lands here having leaked the
        // key in the clear on the first hop. The reassurance has to name the
        // chain it cannot rule out, or that partner is told there is nothing to
        // revoke at exactly the wrong moment.
        assert!(
            advice.contains("chain of redirects"),
            "the advice must not promise more than the final URL can support: \
             {advice}",
        );

        // ...as is a port change on the same host.
        let other_port: reqwest::Url = "https://maker.signet.kaleidoswap.com:8443/v2/x"
            .parse()
            .unwrap();
        assert!(redirect_strips_api_key(&secure, &other_port));
        assert!(api_key_redirect_advice(&secure, &other_port).contains("did not travel"));
    }

    /// An unauthenticated client must keep following redirects and parsing what
    /// comes back.
    ///
    /// The redirect check is new on the GET path, and it guards a credential
    /// nobody without a key has. A caller pointed at a maker that answers `302`
    /// — a vanity host, a trailing-slash normalisation — worked before this
    /// change and has to keep working.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn a_keyless_client_still_follows_a_redirect_off_its_base_url() {
        let body = r#"{"BTC":800000,"L-BTC":1}"#;
        let (elsewhere_url, elsewhere) = capture_one_request_answering(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\
                 \r\n\r\n{body}",
                body.len(),
            )
            .into_bytes(),
        );
        let elsewhere_host = elsewhere_url
            .strip_prefix("http://")
            .and_then(|url| url.strip_suffix("/v2"))
            .unwrap()
            .to_string();
        let (base_url, maker) = capture_one_request_answering(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{elsewhere_host}/v2/chain/heights\r\n\
                 Content-Length: 0\r\n\r\n"
            )
            .into_bytes(),
        );

        let follows_redirects = reqwest::Client::builder().build().unwrap();
        let heights = BoltzApiClientV2::with_client(base_url, follows_redirects, None)
            .get_height()
            .await
            .expect("a keyless client carries nothing a redirect could disclose");
        assert_eq!(heights.btc, 800_000);

        maker.join().unwrap();
        elsewhere.join().unwrap();
    }

    /// `{:?}` on a create response must not print the credential.
    ///
    /// It is the taker's full capability over the swap, and logging a whole
    /// response is the ordinary way to trace one — so the derive would put the
    /// credential wherever those logs go.
    #[test]
    fn debug_redacts_the_swap_auth_credential() {
        let auth = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4";

        let chain: CreateChainResponse =
            serde_json::from_value(chain_create_response_json(Some(auth))).unwrap();
        let rendered = format!("{chain:?}");

        assert!(
            !rendered.contains(auth),
            "the credential must not reach a log line, got:\n{rendered}",
        );
        assert!(
            rendered.contains("swap_auth: Some(<redacted>)"),
            "{rendered}"
        );
        // Everything else still prints, so the redaction did not cost the
        // response its usefulness in a log.
        assert!(
            rendered.contains("01KZZYB138E7C3HZX7Q1YBGAQG"),
            "{rendered}"
        );
        assert!(rendered.contains("claim_details"), "{rendered}");

        let none: CreateChainResponse =
            serde_json::from_value(chain_create_response_json(None)).unwrap();
        assert!(
            format!("{none:?}").contains("swap_auth: None"),
            "whether the maker issued one is still worth seeing",
        );
    }

    /// A 2xx response the SDK cannot deserialize must not put the body in the
    /// error. Every create response carries `swapAuth`, the per-swap taker
    /// credential, so the body is secret material and this error is routinely
    /// logged. The guard does not rest on the struct: the maker sends the field
    /// whether or not the SDK models it, and the value redacted below is the
    /// one serde chose to echo, not the one the schema happened to name.
    #[test]
    fn an_unparseable_success_body_is_described_and_not_quoted() {
        // Field order is the maker's, so keep the body a literal: serde reports
        // the first field it cannot take, which here is `timeoutBlockHeight`.
        const UNMODELLED: &str = "sa_live_9f2c0b7d41e84a6f";
        const OFFENDING: &str = "sa_live_0f7ac2b19e6482d5";
        let body = format!(
            r#"{{"id":"01KZZYB138E7C3HZX7Q1YBGAQG",
                 "swapAuth":"{UNMODELLED}",
                 "timeoutBlockHeight":"{OFFENDING}"}}"#
        );

        let err = BoltzApiClientV2::json_from_body::<CreateReverseResponse>(
            reqwest::StatusCode::CREATED,
            body,
        )
        .expect_err("a u32 field given a string cannot deserialize");

        assert_eq!(err.name(), "HTTPResponseBodyInvalid");
        let message = err.message();

        // serde echoes only the value it choked on, so the credential in the
        // sibling field never reaches the message; the one it did choke on is
        // exactly what would have gone out verbatim.
        assert!(!message.contains(UNMODELLED), "leaked the body: {message}");
        assert!(
            !message.contains(OFFENDING),
            "leaked the offending value: {message}"
        );
        assert!(message.contains("<redacted>"), "{message}");

        // What is left is the diagnosis: the mismatch, and the field names the
        // maker sent — including the one the SDK does not model.
        assert!(message.contains("invalid type"), "{message}");
        assert!(message.contains("u32"), "{message}");
        assert!(
            message.contains("body keys: id, swapAuth, timeoutBlockHeight"),
            "{message}"
        );
    }

    /// The status decides which failure this is. A maker rejection keeps its
    /// body, because `invalid_swap_auth` or `pair_hash_mismatch` is the whole
    /// diagnosis and callers read it; a 2xx the SDK cannot parse is schema skew
    /// and reports as such, naming the field serde missed.
    #[test]
    fn a_maker_rejection_and_a_schema_mismatch_are_different_errors() {
        let rejected = BoltzApiClientV2::json_from_body::<CreateReverseResponse>(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"error":"invalid_swap_auth"}"#.to_owned(),
        )
        .expect_err("401 is not a success status");

        assert_eq!(rejected.name(), "HTTPStatusNotSuccess");
        assert!(
            rejected.message().contains("invalid_swap_auth"),
            "{}",
            rejected.message()
        );

        let skewed = BoltzApiClientV2::json_from_body::<CreateReverseResponse>(
            reqwest::StatusCode::CREATED,
            r#"{"id":"01KZZYB138E7C3HZX7Q1YBGAQG"}"#.to_owned(),
        )
        .expect_err("a create response missing every other field cannot deserialize");

        assert_eq!(skewed.name(), "HTTPResponseBodyInvalid");
        assert!(
            skewed.message().contains("missing field"),
            "{}",
            skewed.message()
        );
    }

    /// serde renders an unknown enum variant in *backticks* — the same way it
    /// renders a field or type name out of the schema — so a body value in an
    /// enum-typed field is not covered by redacting double-quoted runs alone.
    /// `SwapRestoreResponse::swap_type` makes that reachable on a real response.
    #[test]
    fn a_body_value_serde_reports_in_backticks_is_redacted_too() {
        const CREDENTIAL: &str = "sa_live_9f2c0b7d41e84a6f";
        let body = format!(
            r#"{{"id":"01KZZYB138E7C3HZX7Q1YBGAQG","type":"{CREDENTIAL}",
                 "status":"swap.created","createdAt":1,"from":"BTC","to":"BTC"}}"#
        );

        let err =
            BoltzApiClientV2::json_from_body::<SwapRestoreResponse>(reqwest::StatusCode::OK, body)
                .expect_err("no variant of SwapRestoreType matches");

        assert_eq!(err.name(), "HTTPResponseBodyInvalid");
        let message = err.message();

        assert!(
            !message.contains(CREDENTIAL),
            "leaked in backticks: {message}"
        );
        assert!(
            message.contains("unknown variant `<redacted>`"),
            "{message}"
        );
        // The variant names in the same message come from the schema, not the
        // body, and are the half of it worth reading.
        assert!(
            message.contains("expected one of `reverse`, `submarine`, `chain`"),
            "{message}"
        );
    }

    /// The two kinds of delimited run, side by side. `{:?}` escapes a quote
    /// inside the string it echoes, so the redactor has to skip the escape
    /// rather than read it as the run's end; and a backticked run survives only
    /// when the body does not claim it as a value.
    #[test]
    fn redaction_spares_schema_names_and_covers_escaped_quotes() {
        let body = serde_json::json!({"type": "reverse", "nested": ["from-the-body"]});
        let mut scalars = HashSet::new();
        BoltzApiClientV2::collect_scalars(&body, &mut scalars);

        // A name the schema owns is kept; the same run is redacted once the body
        // claims that string — here `reverse`, which is both.
        assert_eq!(
            BoltzApiClientV2::redact_body_values(
                "missing field `swapTree` at line 1 column 35",
                &scalars
            ),
            "missing field `swapTree` at line 1 column 35",
        );
        assert_eq!(
            BoltzApiClientV2::redact_body_values(
                "unknown variant `reverse`, expected one of `submarine`, `chain`",
                &scalars
            ),
            "unknown variant `<redacted>`, expected one of `submarine`, `chain`",
        );
        // Nesting is no escape from the scalar sweep.
        assert_eq!(
            BoltzApiClientV2::redact_body_values("unknown variant `from-the-body`", &scalars),
            "unknown variant `<redacted>`",
        );
        // A quoted run goes regardless, escapes and all.
        assert_eq!(
            BoltzApiClientV2::redact_body_values(
                r#"invalid type: string "before\"after", expected u32 at line 1 column 9"#,
                &scalars
            ),
            r#"invalid type: string "<redacted>", expected u32 at line 1 column 9"#,
        );
    }

    #[test]
    fn default_maker_endpoints_match_their_network() {
        assert_eq!(
            BoltzApiClientV2::default(Network::Signet).unwrap().base_url,
            KALEIDOSWAP_SIGNET_URL_V2,
        );
        assert_eq!(
            BoltzApiClientV2::default(Network::Regtest)
                .unwrap()
                .base_url,
            BOLTZ_REGTEST,
        );
        // No mainnet maker is live, and we run no testnet3 maker at all —
        // signet is our testing network. Neither may fall back to Boltz.
        assert!(BoltzApiClientV2::default(Network::Mainnet).is_err());
        assert!(BoltzApiClientV2::default(Network::Testnet).is_err());
        // The Boltz endpoints stay reachable, but only when named explicitly.
        assert_eq!(
            BoltzApiClientV2::new(BOLTZ_TESTNET_URL_V2.to_string(), None).base_url,
            BOLTZ_TESTNET_URL_V2,
        );
    }

    #[macros::async_test_all]
    async fn test_get_fee_estimation() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let result = client.get_fee_estimation().await;
        assert!(result.is_ok(), "Failed to get fee estimation");
    }

    #[macros::async_test_all]
    async fn test_get_height() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let result = client.get_height().await;
        assert!(result.is_ok(), "Failed to get height");
    }

    // Hits the live mainnet swap/restore endpoint with the swap-master xpub
    // derived from a known wallet mnemonic, and prints what boltz returns.
    // Run: cargo test test_swap_restore_endpoint_print -- --nocapture
    #[macros::async_test_all]
    async fn test_swap_restore_endpoint_print() {
        let wallet_mnemonic =
            "slogan prevent affair connect autumn crop together earn track ribbon horn copy";
        let swap_master_key =
            crate::util::secrets::SwapMasterKey::new(wallet_mnemonic, None, Network::Mainnet)
                .unwrap();
        let xpub = swap_master_key.get_master_xpub().to_string();
        println!("SWAP_RESTORE_TEST xpub: {xpub}");

        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let responses = client
            .post_swap_restore(&xpub, Some("m".to_string()), Some(100))
            .await
            .unwrap();
        println!("SWAP_RESTORE_TEST returned {} swaps", responses.len());
        for r in &responses {
            println!(
                "SWAP_RESTORE_TEST   {} type={:?} status={} {}->{}",
                r.id, r.swap_type, r.status, r.from, r.to
            );
        }
    }

    // Creates a fresh BTC->L-BTC chain swap at swap-key indexes 0 (refund) and
    // 1 (claim) using the seed's xpub-derived keys, then immediately calls
    // swap/restore (and swap/restore/index) with the same xpub to see whether
    // boltz matches the just-registered leaf pubkeys.
    // Run: cargo test test_create_chain_then_restore -- --nocapture
    #[macros::async_test_all]
    async fn test_create_chain_then_restore() {
        use crate::util::secrets::{Preimage, SwapMasterKey};
        let wallet_mnemonic =
            "slogan prevent affair connect autumn crop together earn track ribbon horn copy";
        let smk = SwapMasterKey::new(wallet_mnemonic, None, Network::Mainnet).unwrap();
        let xpub = smk.get_master_xpub().to_string();
        let refund_kps = smk.derive_swapkey(0).unwrap();
        let claim_kps = smk.derive_swapkey(1).unwrap();
        let refund_public_key = PublicKey {
            inner: refund_kps.public_key(),
            compressed: true,
        };
        let claim_public_key = PublicKey {
            inner: claim_kps.public_key(),
            compressed: true,
        };
        let preimage = Preimage::from_swap_key(&claim_kps);
        println!("CREATE_RESTORE xpub         : {xpub}");
        println!("CREATE_RESTORE refund pubkey: {refund_public_key} (index 0)");
        println!("CREATE_RESTORE claim pubkey : {claim_public_key} (index 1)");

        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let req = CreateChainRequest {
            from: "BTC".to_string(),
            to: "L-BTC".to_string(),
            preimage_hash: preimage.sha256,
            claim_public_key: Some(claim_public_key),
            refund_public_key: Some(refund_public_key),
            user_lock_amount: Some(100_000),
            server_lock_amount: None,
            pair_hash: None,
            referral_id: None,
            webhook: None,
        };
        let created = client.post_chain_req(req).await;
        match &created {
            Ok(resp) => println!("CREATE_RESTORE created chain swap: {}", resp.id),
            Err(e) => println!("CREATE_RESTORE create FAILED: {e:?}"),
        }
        let created_id = created.ok().map(|r| r.id);

        match client
            .post_swap_restore_index(&xpub, Some("m".to_string()), Some(100))
            .await
        {
            Ok(idx) => println!("CREATE_RESTORE restore/index for xpub = {}", idx.index),
            Err(e) => println!("CREATE_RESTORE restore/index FAILED: {e:?}"),
        }

        let responses = client
            .post_swap_restore(&xpub, Some("m".to_string()), Some(100))
            .await
            .unwrap();
        println!("CREATE_RESTORE restore returned {} swaps", responses.len());
        for r in &responses {
            let ck = r
                .claim_details
                .as_ref()
                .map(|d| d.key_index as i64)
                .unwrap_or(-1);
            let rk = r
                .refund_details
                .as_ref()
                .map(|d| d.key_index as i64)
                .unwrap_or(-1);
            println!(
                "CREATE_RESTORE   {} type={:?} status={} {}->{} claimIdx={} refundIdx={}",
                r.id, r.swap_type, r.status, r.from, r.to, ck, rk
            );
        }
        if let Some(id) = created_id {
            let found = responses.iter().any(|r| r.id == id);
            println!("CREATE_RESTORE just-created swap {id} found in restore: {found}");
        }
    }

    #[macros::async_test_all]
    async fn test_get_submarine_pairs() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let result = client.get_submarine_pairs().await;
        assert!(result.is_ok(), "Failed to get submarine pairs");
    }

    #[macros::async_test_all]
    async fn test_get_reverse_pairs() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let result = client.get_reverse_pairs().await;
        assert!(result.is_ok(), "Failed to get reverse pairs");
    }

    #[macros::async_test_all]
    async fn test_get_chain_pairs() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let result = client.get_chain_pairs().await;
        assert!(result.is_ok(), "Failed to get chain pairs");
    }

    #[macros::async_test_all]
    #[ignore]
    async fn test_get_submarine_claim_tx_details() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "G6c6GJJY8eXz".to_string();
        let result = client.get_submarine_claim_tx_details(&id).await;
        assert!(
            result.is_ok(),
            "Failed to get submarine claim transaction details"
        );
    }

    #[macros::async_test_all]
    #[ignore]
    async fn test_get_chain_claim_tx_details() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "3BIJf8UqGaSC".to_string();
        let result = client.get_chain_claim_tx_details(&id).await;
        assert!(
            result.is_ok(),
            "Failed to get chain claim transaction details"
        );
    }

    #[macros::async_test_all]
    #[ignore]
    async fn test_get_reverse_tx() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "G6c6GJJY8eXz";
        let result = client.get_reverse_tx(id).await;
        assert!(result.is_ok(), "Failed to get reverse transaction");
    }

    #[macros::async_test_all]
    #[ignore]
    async fn test_get_submarine_tx() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "G6c6GJJY8eXz";
        let result = client.get_submarine_tx(id).await;
        assert!(result.is_ok(), "Failed to get submarine transaction");
    }

    #[macros::async_test_all]
    async fn test_get_chain_txs() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "G6c6GJJY8eXz";
        let result = client.get_chain_txs(id).await;
        assert!(result.is_ok(), "Failed to get chain transactions");
    }

    #[macros::async_test_all]
    async fn test_get_swap() {
        let client = BoltzApiClientV2::new(BOLTZ_MAINNET_URL_V2.to_string(), None);
        let id = "G6c6GJJY8eXz";
        let result = client.get_swap(id).await;
        assert!(result.is_ok(), "Failed to get swap status");
    }
}
