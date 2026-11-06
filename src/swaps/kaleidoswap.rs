//! KaleidoSwap — the high-level surface.
//!
//! One verb per step — `quote`, `start`, `resume` — over the routes the maker
//! serves on Boltz `/v2`: Lightning to on-chain BTC or L-BTC (reverse) and
//! on-chain BTC or L-BTC to Lightning (submarine). The protocol a quote rides
//! on is reported, never chosen by the caller. The engine below stays public;
//! this layer holds no key material of its own: keys come from the injected
//! [`SwapSigner`], and every byte a swap needs to finish or unwind after a
//! restart goes through the injected [`SwapStore`] before the irreversible
//! step it precedes.
//!
//! The maker and the chain steps sit behind two seams, [`MakerApi`] and
//! [`SwapFlows`], so the phase machine is tested without a network and every
//! binding inherits the same one.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures_util::lock::Mutex as AsyncMutex;

use bitcoin::secp256k1::Keypair;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::network::esplora::{EsploraBitcoinClient, EsploraLiquidClient};
use crate::network::{BitcoinChain, Chain as ChainKind, Currency, LiquidChain, Network};
use crate::swaps::boltz::{
    BoltzApiClientV2, CreateReverseRequest, CreateReverseResponse, CreateSubmarineRequest,
    CreateSubmarineResponse, GetReversePairsResponse, GetSubmarinePairsResponse, PairMinerFees,
    ReversePair, SubmarinePair,
};
use crate::swaps::{
    BtcLikeTransaction, ChainClient, SwapScript, SwapTransactionParams, TransactionOptions,
};
use crate::util::fees::Fee;
use crate::util::secrets::{Preimage, SwapMasterKey};
use crate::util::{sleep, unix_now};

// ---------------------------------------------------------------------------
// Assets and amounts

/// The layer an asset lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Bitcoin,
    Lightning,
    Liquid,
    Arkade,
    Spark,
}

/// What a swap pays or receives, and where it lives. Naming the layer is
/// what makes a wrong-network destination a local error instead of an
/// unrecoverable payout.
///
/// On the wire an asset is `<layer>:<asset>`: `lightning:BTC`, `liquid:USDT`,
/// `bitcoin:rgb:<contract>`, `spark:<token>`. RGB contracts and Spark tokens
/// are named by id, so a new asset is data, not a new variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Asset {
    BtcOnchain,
    BtcLightning,
    BtcLiquid,
    UsdtLiquid,
    BtcArkade,
    BtcSpark,
    /// An RGB asset on Bitcoin L1. `contract_id` is the full `rgb:…` id.
    RgbOnchain {
        contract_id: String,
    },
    /// An RGB asset over RGB Lightning channels.
    RgbLightning {
        contract_id: String,
    },
    /// A token on Spark, by its `btkn…` identifier.
    SparkToken {
        token_id: String,
    },
}

impl Asset {
    pub fn layer(&self) -> Layer {
        match self {
            Asset::BtcOnchain | Asset::RgbOnchain { .. } => Layer::Bitcoin,
            Asset::BtcLightning | Asset::RgbLightning { .. } => Layer::Lightning,
            Asset::BtcLiquid | Asset::UsdtLiquid => Layer::Liquid,
            Asset::BtcArkade => Layer::Arkade,
            Asset::BtcSpark | Asset::SparkToken { .. } => Layer::Spark,
        }
    }

    /// The settlement-layer code KaleidoSwap's market API and wallet-engine
    /// use (`BTC_LN`, `RGB_L1`, …).
    pub fn code(&self) -> &'static str {
        match self {
            Asset::BtcOnchain => "BTC_L1",
            Asset::BtcLightning => "BTC_LN",
            Asset::BtcLiquid => "BTC_LIQUID",
            Asset::UsdtLiquid => "LIQUID_ASSET",
            Asset::BtcArkade => "BTC_ARKADE",
            Asset::BtcSpark => "BTC_SPARK",
            Asset::RgbOnchain { .. } => "RGB_L1",
            Asset::RgbLightning { .. } => "RGB_LN",
            Asset::SparkToken { .. } => "SPARK_SPARK",
        }
    }

    /// The maker's Boltz `/v2` catalogue symbol, on the routes this layer
    /// creates swaps for.
    fn boltz_symbol(&self) -> Result<&'static str, SwapError> {
        match self {
            Asset::BtcOnchain | Asset::BtcLightning => Ok("BTC"),
            Asset::BtcLiquid => Ok("L-BTC"),
            other => Err(SwapError::new(
                SwapErrorCode::PairUnavailable,
                format!("{other} is not started by this layer"),
            )),
        }
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Asset::BtcOnchain => f.write_str("bitcoin:BTC"),
            Asset::BtcLightning => f.write_str("lightning:BTC"),
            Asset::BtcLiquid => f.write_str("liquid:BTC"),
            Asset::UsdtLiquid => f.write_str("liquid:USDT"),
            Asset::BtcArkade => f.write_str("arkade:BTC"),
            Asset::BtcSpark => f.write_str("spark:BTC"),
            Asset::RgbOnchain { contract_id } => write!(f, "bitcoin:{contract_id}"),
            Asset::RgbLightning { contract_id } => write!(f, "lightning:{contract_id}"),
            Asset::SparkToken { token_id } => write!(f, "spark:{token_id}"),
        }
    }
}

impl std::str::FromStr for Asset {
    type Err = SwapError;
    fn from_str(s: &str) -> Result<Self, SwapError> {
        let unknown = || {
            SwapError::new(
                SwapErrorCode::PairUnavailable,
                format!(
                    "unknown asset {s}; expected <layer>:<asset>, such as lightning:BTC, \
                     bitcoin:BTC, liquid:BTC, liquid:USDT, arkade:BTC, spark:BTC, \
                     bitcoin:rgb:<contract> or spark:<token>"
                ),
            )
        };
        let (layer, asset) = s.split_once(':').ok_or_else(unknown)?;
        let id_ok = |id: &str| !id.is_empty() && !id.contains(char::is_whitespace);
        let rgb = |id: &str| id.strip_prefix("rgb:").is_some_and(id_ok);
        Ok(match (layer, asset) {
            ("bitcoin", "BTC") => Asset::BtcOnchain,
            ("lightning", "BTC") => Asset::BtcLightning,
            ("liquid", "BTC") => Asset::BtcLiquid,
            ("liquid", "USDT") => Asset::UsdtLiquid,
            ("arkade", "BTC") => Asset::BtcArkade,
            ("spark", "BTC") => Asset::BtcSpark,
            ("bitcoin", id) if rgb(id) => Asset::RgbOnchain {
                contract_id: id.to_string(),
            },
            ("lightning", id) if rgb(id) => Asset::RgbLightning {
                contract_id: id.to_string(),
            },
            ("spark", id) if id_ok(id) => Asset::SparkToken {
                token_id: id.to_string(),
            },
            _ => return Err(unknown()),
        })
    }
}

impl Serialize for Asset {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse()
            .map_err(|e: SwapError| serde::de::Error::custom(e.message))
    }
}

// ---------------------------------------------------------------------------
// Errors

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SwapErrorCode {
    InvalidAmount,
    AmountOutOfLimits,
    PairUnavailable,
    QuoteExpired,
    WrongNetworkAddress,
    InvalidDestination,
    RefundAddressRequired,
    LockupMismatch,
    MakerUnreachable,
    MakerRejected,
    SwapExpired,
    SwapFailed,
    SwapNotFound,
    StoreFailed,
    Aborted,
    ChainUnreachable,
    /// The BOLT11 a swap to Lightning would pay has expired.
    InvoiceExpired,
}

/// Every failure crosses this boundary coded; the engine's errors are mapped here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapError {
    pub code: SwapErrorCode,
    pub message: String,
    pub swap_id: Option<String>,
}

impl SwapError {
    pub fn new(code: SwapErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            swap_id: None,
        }
    }

    pub fn for_swap(code: SwapErrorCode, message: impl Into<String>, swap_id: &str) -> Self {
        Self {
            code,
            message: message.into(),
            swap_id: Some(swap_id.to_string()),
        }
    }

    /// The engine's error onto a code. A 4xx from the maker is a rejection,
    /// anything else on the HTTP path is unreachability.
    pub fn from_core(error: Error, swap_id: Option<&str>) -> Self {
        let code = match &error {
            Error::HTTPStatusNotSuccess(status, _) if status.is_client_error() => {
                SwapErrorCode::MakerRejected
            }
            Error::HTTPStatusNotSuccess(..)
            | Error::HTTP(_)
            | Error::HTTPResponseBodyInvalid(..) => SwapErrorCode::MakerUnreachable,
            _ => SwapErrorCode::SwapFailed,
        };
        Self {
            code,
            message: format!("{error:?}"),
            swap_id: swap_id.map(str::to_string),
        }
    }
}

impl fmt::Display for SwapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.swap_id {
            Some(id) => write!(f, "{:?} ({id}): {}", self.code, self.message),
            None => write!(f, "{:?}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for SwapError {}

// ---------------------------------------------------------------------------
// Quote

/// What fixes a quote's size. `Pay` and `Receive` are atomic units of that
/// side's asset; `Invoice` is the BOLT11 a swap to Lightning pays, and its
/// amount is the `receive` amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Amount {
    Pay(u64),
    Receive(u64),
    Invoice(String),
}

/// Both sides are the caller's to name; the protocol follows from them and
/// is reported on the [`Quote`], never inferred from an address or invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// What you pay, and where it lives.
    pub pay: Asset,
    /// What you receive, and where it lives.
    pub receive: Asset,
    pub amount: Amount,
}

/// The wire a quote rides on. Informational; the caller never picks one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    BoltzSubmarine,
    BoltzReverse,
    ArkadeRfq,
    PayThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fees {
    pub maker: u64,
    pub miner: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub min: u64,
    pub max: u64,
}

/// What holds the maker to a quote's terms. Carried verbatim into `start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteBinding {
    pub pair_hash: String,
    pub invoice: Option<String>,
    pub zero_conf_limit: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub protocol: Protocol,
    #[serde(rename = "from")]
    pub from_asset: Asset,
    #[serde(rename = "to")]
    pub to_asset: Asset,
    pub from_amount: u64,
    pub to_amount: u64,
    pub fees: Fees,
    pub limits: Limits,
    /// Unix seconds. Accepting after this is a new quote, not this one.
    pub expires_at: u64,
    pub binding: QuoteBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteRefusalReason {
    PairUnavailable,
    AmountBelowMin,
    AmountAboveMax,
    NoLiquidity,
    MakerPaused,
}

/// The maker's answer when it will not trade. A value, not an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    #[serde(rename = "from")]
    pub from_asset: Asset,
    #[serde(rename = "to")]
    pub to_asset: Asset,
    pub reason: QuoteRefusalReason,
    pub limits: Option<Limits>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum QuoteAnswer {
    Quote(Quote),
    Refusal(Refusal),
}

impl QuoteAnswer {
    pub fn quote(self) -> Result<Quote, Refusal> {
        match self {
            QuoteAnswer::Quote(q) => Ok(q),
            QuoteAnswer::Refusal(r) => Err(r),
        }
    }
}

// ---------------------------------------------------------------------------
// Start

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOptions {
    /// Where the `to` asset lands: an address on its chain. Validated against
    /// `to` and the network before anything is sent. Omitted when the quote
    /// was priced against an invoice — that invoice is the destination.
    pub destination: Option<String>,
    /// Where a refund lands if the swap unwinds. Required when `from` is an
    /// on-chain asset; ignored for Lightning, which refunds by not settling.
    pub refund_address: Option<String>,
}

// ---------------------------------------------------------------------------
// Swap lifecycle

/// One state machine over every protocol. The engine's per-protocol statuses
/// (`transaction.confirmed`, `invoice.settled`, …) map onto these; the raw one
/// stays readable on [`SwapRecord::raw_status`].
///
/// ```text
///   created ─▶ awaiting_funding ─▶ funded ─▶ locked ─▶ claiming ─▶ settled
///                                               │
///                                               ├─▶ refundable ─▶ refunding ─▶ refunded
///                                               └─▶ failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapPhase {
    Created,
    AwaitingFunding,
    Funded,
    Locked,
    Claiming,
    Settled,
    Refundable,
    Refunding,
    Refunded,
    Failed,
}

impl SwapPhase {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            SwapPhase::Settled | SwapPhase::Refunded | SwapPhase::Failed
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SwapPhase::Created => "created",
            SwapPhase::AwaitingFunding => "awaiting_funding",
            SwapPhase::Funded => "funded",
            SwapPhase::Locked => "locked",
            SwapPhase::Claiming => "claiming",
            SwapPhase::Settled => "settled",
            SwapPhase::Refundable => "refundable",
            SwapPhase::Refunding => "refunding",
            SwapPhase::Refunded => "refunded",
            SwapPhase::Failed => "failed",
        }
    }
}

/// The maker's create response, as the engine parsed it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "kebab-case")]
pub enum CreateResponse {
    BoltzReverse(CreateReverseResponse),
    BoltzSubmarine(CreateSubmarineResponse),
}

/// Everything a swap needs to finish or unwind. Never logged: `response`
/// carries `swap_auth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapState {
    pub swap_index: u64,
    pub response: CreateResponse,
    pub destination: String,
    pub refund_address: Option<String>,
    /// Set once the claim's broadcast is known to have reached the chain.
    pub claim_txid: Option<String>,
    /// Set once the refund's broadcast is known to have reached the chain.
    pub refund_txid: Option<String>,
    /// A claim signed and saved before its broadcast. A broadcast that the
    /// process does not live to record is finished from this on the next
    /// tick instead of signing a second spend of the same lockup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_claim: Option<SignedTx>,
    /// A refund signed and saved before its broadcast, as `pending_claim`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_refund: Option<SignedTx>,
    /// Whether a lockup from us was ever seen — decides `failed` vs
    /// `refundable` on expiry.
    #[serde(default)]
    pub funded_seen: bool,
    /// The largest lockup the maker accepts unconfirmed, as the quote this
    /// swap was started from carried it. `None` on records written before
    /// the limit was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_conf_limit: Option<u64>,
}

/// A claim or refund this layer signed: what it broadcasts, and what it
/// rebroadcasts or looks up if it cannot tell whether the broadcast landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedTx {
    pub txid: String,
    /// The consensus-encoded transaction.
    pub hex: String,
}

/// `Network` as the lowercase names every binding uses (`"signet"`), read
/// back from either spelling.
mod network_name {
    use super::Network;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(network: &Network, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match network {
            Network::Mainnet => "mainnet",
            Network::Testnet => "testnet",
            Network::Signet => "signet",
            Network::Regtest => "regtest",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Network, D::Error> {
        let name = String::deserialize(d)?;
        match name.to_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            "signet" => Ok(Network::Signet),
            "regtest" => Ok(Network::Regtest),
            other => Err(serde::de::Error::custom(format!("unknown network {other}"))),
        }
    }
}

/// What the store holds. Serialize it verbatim; nothing in it needs
/// interpretation by the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRecord {
    pub id: String,
    pub protocol: Protocol,
    #[serde(with = "network_name")]
    pub network: Network,
    pub maker_url: String,
    #[serde(rename = "from")]
    pub from_asset: Asset,
    #[serde(rename = "to")]
    pub to_asset: Asset,
    pub phase: SwapPhase,
    /// The protocol's own status string, for logs and support.
    pub raw_status: String,
    /// Unix seconds.
    pub created_at: u64,
    pub updated_at: u64,
    pub state: SwapState,
}

/// What the caller acts on. Only `invoice` and `funding_address` are ever
/// needed; the rest is for showing the user where things stand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapAction {
    /// Pay this to fund a swap from Lightning.
    pub invoice: Option<String>,
    /// Send exactly `funding_amount` here to fund a swap from an on-chain asset.
    pub funding_address: Option<String>,
    pub funding_amount: Option<u64>,
    /// The maker's BIP21 URI for the same funding: set only when it names
    /// exactly `funding_address` and `funding_amount` and nothing a wallet
    /// could pay instead, such as a `lightning` parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bip21: Option<String>,
    /// The largest lockup the maker accepts unconfirmed. Zero means the
    /// maker waits for a confirmation before acting; `None` that the limit
    /// is not known.
    pub zero_conf_limit: Option<u64>,
}

impl SwapRecord {
    pub fn action(&self) -> SwapAction {
        match &self.state.response {
            CreateResponse::BoltzReverse(r) => SwapAction {
                invoice: r.invoice.clone(),
                ..Default::default()
            },
            CreateResponse::BoltzSubmarine(r) => SwapAction {
                funding_address: Some(r.address.clone()),
                funding_amount: Some(r.expected_amount),
                bip21: checked_bip21(&self.from_asset, self.network, r),
                zero_conf_limit: if r.accept_zero_conf {
                    self.state.zero_conf_limit
                } else {
                    Some(0)
                },
                ..Default::default()
            },
        }
    }
}

/// `value`, a BIP21 `amount` in BTC, in satoshis. More than eight decimals,
/// a sign or an exponent is not an amount.
fn btc_decimal_to_sat(value: &str) -> Option<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty())
        || !digits(whole)
        || !digits(fraction)
        || fraction.len() > 8
    {
        return None;
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: u64 = format!("{fraction:0<8}").parse().ok()?;
    whole.checked_mul(100_000_000)?.checked_add(fraction)
}

/// The maker's BIP21 for a submarine lockup, when every part of it is one
/// this layer verified: the scheme of the funding chain, the lockup address
/// `verify_lockup` rebuilt, the exact lockup amount and, on Liquid, L-BTC.
/// Labels and messages pass; any other parameter withholds the URI, since a
/// wallet may act on it instead of the address.
fn checked_bip21(from: &Asset, network: Network, r: &CreateSubmarineResponse) -> Option<String> {
    let uri = r.bip21.trim();
    let (scheme, rest) = uri.split_once(':')?;
    let scheme_ok = match from {
        Asset::BtcOnchain => scheme.eq_ignore_ascii_case("bitcoin"),
        Asset::BtcLiquid => {
            scheme.eq_ignore_ascii_case("liquidnetwork")
                || scheme.eq_ignore_ascii_case("liquidtestnet")
        }
        _ => false,
    };
    let (address, query) = rest.split_once('?').unwrap_or((rest, ""));
    if !scheme_ok || address != r.address {
        return None;
    }
    let mut amount_ok = false;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key.to_ascii_lowercase().as_str() {
            "amount" => amount_ok = btc_decimal_to_sat(value) == Some(r.expected_amount),
            "assetid" if *from == Asset::BtcLiquid => {
                let lbtc = LiquidChain::from(network).bitcoin().to_string();
                if !value.eq_ignore_ascii_case(&lbtc) {
                    return None;
                }
            }
            "label" | "message" => {}
            _ => return None,
        }
    }
    amount_ok.then(|| uri.to_string())
}

// ---------------------------------------------------------------------------
// Injected dependencies

/// Where the SDK keeps everything a swap needs to finish or unwind after a
/// restart. Every write lands before the irreversible step it precedes.
#[macros::async_trait]
pub trait SwapStore: Send + Sync {
    async fn get(&self, id: &str) -> Result<Option<SwapRecord>, SwapError>;
    async fn put(&self, record: &SwapRecord) -> Result<(), SwapError>;
    async fn list(&self) -> Result<Vec<SwapRecord>, SwapError>;
    /// Hands out the next BIP85 child index, exactly once. An index reused is
    /// claim and refund material reused, so this has to be atomic for the store.
    async fn next_index(&self) -> Result<u64, SwapError>;
}

/// Derives per-swap material from a root the caller owns. The SDK never sees
/// a mnemonic through it.
#[macros::async_trait]
pub trait SwapSigner: Send + Sync {
    async fn derive_key(&self, index: u64) -> Result<Keypair, SwapError>;
    async fn derive_preimage(&self, index: u64) -> Result<Preimage, SwapError>;
}

/// The maker, as this layer needs it. Implemented by [`BoltzApiClientV2`];
/// tests implement it in memory.
#[macros::async_trait]
pub trait MakerApi: Send + Sync {
    async fn reverse_pairs(&self) -> Result<GetReversePairsResponse, Error>;
    async fn submarine_pairs(&self) -> Result<GetSubmarinePairsResponse, Error>;
    async fn create_reverse(
        &self,
        req: CreateReverseRequest,
    ) -> Result<CreateReverseResponse, Error>;
    async fn create_submarine(
        &self,
        req: &CreateSubmarineRequest,
    ) -> Result<CreateSubmarineResponse, Error>;
    async fn swap_status(&self, id: &str) -> Result<String, Error>;
}

#[macros::async_trait]
impl MakerApi for BoltzApiClientV2 {
    async fn reverse_pairs(&self) -> Result<GetReversePairsResponse, Error> {
        self.get_reverse_pairs().await
    }
    async fn submarine_pairs(&self) -> Result<GetSubmarinePairsResponse, Error> {
        self.get_submarine_pairs().await
    }
    async fn create_reverse(
        &self,
        req: CreateReverseRequest,
    ) -> Result<CreateReverseResponse, Error> {
        self.post_reverse_req(req).await
    }
    async fn create_submarine(
        &self,
        req: &CreateSubmarineRequest,
    ) -> Result<CreateSubmarineResponse, Error> {
        self.post_swap_req(req).await
    }
    async fn swap_status(&self, id: &str) -> Result<String, Error> {
        Ok(self.get_swap(id).await?.status)
    }
}

/// The on-chain steps, as a seam. [`DefaultFlows`] builds and broadcasts
/// through [`SwapScript`]; tests replace it so nothing here needs a chain.
#[macros::async_trait]
pub trait SwapFlows: Send + Sync {
    /// Checks the maker's create response against what we asked for and
    /// against our own keys: the lockup tree and address, the hashlock
    /// against `preimage`, and for a reverse swap the invoice's payment hash
    /// and network. Runs before anything is persisted or funded.
    fn verify_lockup(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<(), Error>;
    /// Builds and signs the claim of a reverse swap's lockup to its
    /// destination. Broadcasts nothing: the caller saves it first.
    async fn sign_claim(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<SignedTx, Error>;
    /// Builds and signs the refund of a submarine swap's lockup to its
    /// refund address. Broadcasts nothing: the caller saves it first.
    async fn sign_refund(&self, record: &SwapRecord, keys: &Keypair) -> Result<SignedTx, Error>;
    /// Broadcasts a transaction `sign_claim` or `sign_refund` produced.
    async fn broadcast(&self, record: &SwapRecord, tx: &SignedTx) -> Result<(), Error>;
    /// Whether the chain client has `tx`, in the mempool or in a block.
    async fn tx_seen(&self, record: &SwapRecord, tx: &SignedTx) -> Result<bool, Error>;
    /// The tip of the chain the swap's lockup lives on, from the trusted
    /// chain client, never the maker's `/chain/heights`.
    async fn lockup_tip_height(&self, record: &SwapRecord) -> Result<u32, Error>;
    /// Whether `txid`, a claim or refund this layer broadcast, is in a block.
    async fn tx_confirmed(&self, record: &SwapRecord, txid: &str) -> Result<bool, Error>;
    /// Whether an unspent output pays the swap's lockup address. This, not
    /// whether a status poll happened to see the funding, decides that an
    /// expired submarine swap has something to refund.
    async fn lockup_unspent(&self, record: &SwapRecord) -> Result<bool, Error>;
}

/// A [`SwapSigner`] over a [`SwapMasterKey`].
pub struct MnemonicSigner {
    master: SwapMasterKey,
}

impl MnemonicSigner {
    pub fn new(master: SwapMasterKey) -> Self {
        Self { master }
    }

    pub fn from_swap_mnemonic(
        mnemonic: &str,
        passphrase: Option<&str>,
        network: Network,
    ) -> Result<Self, SwapError> {
        let master = SwapMasterKey::from_mnemonic(mnemonic, passphrase, network)
            .map_err(|e| SwapError::from_core(e, None))?;
        Ok(Self { master })
    }
}

#[macros::async_trait]
impl SwapSigner for MnemonicSigner {
    async fn derive_key(&self, index: u64) -> Result<Keypair, SwapError> {
        self.master
            .derive_swapkey(index)
            .map_err(|e| SwapError::from_core(e, None))
    }
    async fn derive_preimage(&self, index: u64) -> Result<Preimage, SwapError> {
        let keys = self.derive_key(index).await?;
        Ok(Preimage::from_swap_key(&keys))
    }
}

/// In-memory store — tests and throwaway scripts. Nothing survives the process.
#[derive(Default)]
pub struct MemoryStore {
    inner: std::sync::Mutex<(std::collections::BTreeMap<String, SwapRecord>, u64)>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[macros::async_trait]
impl SwapStore for MemoryStore {
    async fn get(&self, id: &str) -> Result<Option<SwapRecord>, SwapError> {
        Ok(self.inner.lock().unwrap().0.get(id).cloned())
    }
    async fn put(&self, record: &SwapRecord) -> Result<(), SwapError> {
        self.inner
            .lock()
            .unwrap()
            .0
            .insert(record.id.clone(), record.clone());
        Ok(())
    }
    async fn list(&self) -> Result<Vec<SwapRecord>, SwapError> {
        Ok(self.inner.lock().unwrap().0.values().cloned().collect())
    }
    async fn next_index(&self) -> Result<u64, SwapError> {
        let mut guard = self.inner.lock().unwrap();
        let index = guard.1;
        guard.1 += 1;
        Ok(index)
    }
}

/// A [`SwapStore`] over a directory: one JSON file per swap plus `index`
/// for the BIP85 counter. Writes go through a rename so a crash mid-write
/// leaves the previous file, never a torn one; files are owner-readable only
/// because records carry `swap_auth`. Single-process: two instances on one
/// directory would hand out the same index.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub struct FileStore {
    dir: std::path::PathBuf,
    index_lock: std::sync::Mutex<()>,
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
impl FileStore {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            index_lock: std::sync::Mutex::new(()),
        }
    }

    fn store_err(e: impl fmt::Display) -> SwapError {
        SwapError::new(SwapErrorCode::StoreFailed, e.to_string())
    }

    fn file(&self, name: &str) -> Result<std::path::PathBuf, SwapError> {
        if !is_plain_id(name.trim_end_matches(".json")) {
            return Err(Self::store_err(format!(
                "refusing to use {name} as a file name"
            )));
        }
        Ok(self.dir.join(name))
    }

    fn write(&self, name: &str, body: &str) -> Result<(), SwapError> {
        use std::io::Write;
        std::fs::create_dir_all(&self.dir).map_err(Self::store_err)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700));
        }
        let target = self.file(name)?;
        // Unique per call and created exclusively, so two writes of one record
        // never share a temporary file.
        static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self
            .dir
            .join(format!("{name}.{}.{seq}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&tmp).map_err(Self::store_err)?;
        f.write_all(body.as_bytes()).map_err(Self::store_err)?;
        f.sync_all().map_err(Self::store_err)?;
        std::fs::rename(&tmp, &target).map_err(Self::store_err)
    }

    fn read(&self, name: &str) -> Result<Option<String>, SwapError> {
        match std::fs::read_to_string(self.file(name)?) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Self::store_err(e)),
        }
    }
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
#[macros::async_trait]
impl SwapStore for FileStore {
    async fn get(&self, id: &str) -> Result<Option<SwapRecord>, SwapError> {
        match self.read(&format!("{id}.json"))? {
            Some(body) => serde_json::from_str(&body)
                .map(Some)
                .map_err(Self::store_err),
            None => Ok(None),
        }
    }
    async fn put(&self, record: &SwapRecord) -> Result<(), SwapError> {
        let body = serde_json::to_string_pretty(record).map_err(Self::store_err)?;
        self.write(&format!("{}.json", record.id), &body)
    }
    async fn list(&self) -> Result<Vec<SwapRecord>, SwapError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(Self::store_err(e)),
        };
        let mut records = vec![];
        for entry in entries {
            let path = entry.map_err(Self::store_err)?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let body = std::fs::read_to_string(&path).map_err(Self::store_err)?;
            records.push(serde_json::from_str(&body).map_err(Self::store_err)?);
        }
        Ok(records)
    }
    async fn next_index(&self) -> Result<u64, SwapError> {
        let _guard = self.index_lock.lock().unwrap();
        let current: u64 = match self.read("index")? {
            Some(s) => s.trim().parse().map_err(Self::store_err)?,
            None => 0,
        };
        // The counter advances on disk before the index is handed out, so a
        // crash between the two skips an index rather than reusing one.
        self.write("index", &(current + 1).to_string())?;
        Ok(current)
    }
}

// ---------------------------------------------------------------------------
// Construction

pub struct KaleidoSwapOptions {
    pub network: Network,
    pub store: Arc<dyn SwapStore>,
    pub signer: Arc<dyn SwapSigner>,
    /// The maker's `/v2` base URL. Defaults per network exactly as
    /// [`BoltzApiClientV2::default`] does.
    pub maker_url: Option<String>,
    pub bitcoin_esplora_url: Option<String>,
    pub liquid_esplora_url: Option<String>,
    /// How often a swap in flight is asked for its status. Default 2 s.
    pub poll_interval: Option<Duration>,
    /// Fee rate for the claims and refunds this layer broadcasts. Default 2 sat/vB.
    pub fee_sat_per_vb: Option<f64>,
    /// How long a quote may be held before `start` refuses it. Pair terms are
    /// bound by hash, so this guards staleness, not price. Default 10 minutes.
    pub quote_ttl: Option<Duration>,
    /// Timeout of every request to the maker. Default 30 s.
    pub maker_timeout: Option<Duration>,
}

pub(crate) struct Env {
    network: Network,
    maker_url: String,
    maker: Arc<dyn MakerApi>,
    store: Arc<dyn SwapStore>,
    signer: Arc<dyn SwapSigner>,
    flows: Arc<dyn SwapFlows>,
    poll_interval: Duration,
    quote_ttl: Duration,
    /// One lock per swap id, shared by every handle this instance gives out
    /// on that id. Held only by live handles: an entry whose handles are all
    /// gone is dropped the next time a lock is handed out.
    swap_locks: std::sync::Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
}

impl Env {
    /// The lock every handle on swap `id` shares, so a tick or a refund on
    /// one waits for any other in flight on the same swap.
    fn swap_lock(&self, id: &str) -> Arc<AsyncMutex<()>> {
        let mut locks = self.swap_locks.lock().unwrap();
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            return lock;
        }
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = Arc::new(AsyncMutex::new(()));
        locks.insert(id.to_string(), Arc::downgrade(&lock));
        lock
    }
}

/// The default chain endpoints per network. Signet settles on Mutinynet, so
/// chain access must too. Blockstream's liquidtestnet Esplora rate-limits
/// UTXO lookups; `liquid.network` serves the same chain.
fn default_chain_urls(network: Network) -> Option<(&'static str, &'static str)> {
    match network {
        Network::Signet => Some((
            "https://esplora.signet.kaleidoswap.com",
            "https://liquid.network/liquidtestnet/api",
        )),
        Network::Regtest => Some(("http://localhost:3002", "http://localhost:3003")),
        _ => None,
    }
}

/// [`SwapFlows`] over [`SwapScript`] and Esplora chain access.
pub struct DefaultFlows {
    api: Arc<BoltzApiClientV2>,
    chain: ChainClient,
    fee: Fee,
}

impl DefaultFlows {
    pub fn new(api: Arc<BoltzApiClientV2>, chain: ChainClient, fee_sat_per_vb: f64) -> Self {
        Self {
            api,
            chain,
            fee: Fee::Relative(fee_sat_per_vb),
        }
    }

    fn chain_kind(record: &SwapRecord) -> ChainKind {
        let on_chain = match record.protocol {
            Protocol::BoltzReverse => &record.to_asset,
            _ => &record.from_asset,
        };
        match on_chain.layer() {
            Layer::Liquid => ChainKind::Liquid(LiquidChain::from(record.network)),
            _ => ChainKind::Bitcoin(BitcoinChain::from(record.network)),
        }
    }

    fn script(record: &SwapRecord, keys: &Keypair) -> Result<SwapScript, Error> {
        let pubkey = bitcoin::PublicKey::new(keys.public_key());
        match &record.state.response {
            CreateResponse::BoltzReverse(r) => {
                SwapScript::reverse_from_swap_resp(Self::chain_kind(record), r, pubkey)
            }
            CreateResponse::BoltzSubmarine(r) => {
                SwapScript::submarine_from_swap_resp(Self::chain_kind(record), r, pubkey)
            }
        }
    }

    fn signed(tx: &BtcLikeTransaction) -> SignedTx {
        match tx {
            BtcLikeTransaction::Bitcoin(tx) => SignedTx {
                txid: tx.compute_txid().to_string(),
                hex: bitcoin::consensus::encode::serialize_hex(tx),
            },
            BtcLikeTransaction::Liquid(tx) => SignedTx {
                txid: tx.txid().to_string(),
                hex: elements::encode::serialize_hex(tx),
            },
        }
    }

    fn swap_auth(record: &SwapRecord) -> Option<String> {
        match &record.state.response {
            CreateResponse::BoltzReverse(r) => r.swap_auth.clone(),
            CreateResponse::BoltzSubmarine(r) => r.swap_auth.clone(),
        }
    }

    fn params<'a>(
        &'a self,
        record: &SwapRecord,
        keys: &Keypair,
        output_address: String,
        cooperative: bool,
    ) -> SwapTransactionParams<'a> {
        let mut options = TransactionOptions::default().with_cooperative(cooperative);
        if let Some(auth) = Self::swap_auth(record) {
            options = options.with_swap_auth(auth);
        }
        SwapTransactionParams {
            keys: *keys,
            output_address,
            fee: self.fee,
            swap_id: record.id.clone(),
            chain_client: &self.chain,
            boltz_api: &self.api,
            options: Some(options),
        }
    }
}

#[macros::async_trait]
impl SwapFlows for DefaultFlows {
    fn verify_lockup(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<(), Error> {
        Self::script(record, keys)?;
        let pubkey = bitcoin::PublicKey::new(keys.public_key());
        let chain = Self::chain_kind(record);
        let on_chain = match record.protocol {
            Protocol::BoltzReverse => &record.to_asset,
            _ => &record.from_asset,
        };
        let currency = Some(match on_chain {
            Asset::BtcLiquid => Currency::LBtc,
            _ => Currency::Btc,
        });
        match &record.state.response {
            CreateResponse::BoltzReverse(r) => {
                // The invoice is paid from Lightning on this network; the
                // HTLC lives on the destination chain.
                let source = ChainKind::Bitcoin(BitcoinChain::from(record.network));
                r.validate_with_source_and_asset_context(
                    preimage, &pubkey, source, chain, currency, None,
                )
            }
            CreateResponse::BoltzSubmarine(r) => r.validate_with_currency_and_asset_context(
                &record.state.destination,
                &pubkey,
                chain,
                currency,
                None,
            ),
        }
    }

    async fn sign_claim(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<SignedTx, Error> {
        let script = Self::script(record, keys)?;
        let destination = record.state.destination.clone();
        // The maker co-signs Bitcoin claims but not every Liquid lockup; the
        // script path is always available, so fall back to it rather than
        // leave a claimable lockup behind.
        let tx = match script
            .construct_claim(
                preimage,
                self.params(record, keys, destination.clone(), true),
            )
            .await
        {
            Ok(tx) => tx,
            Err(e) if format!("{e:?}").to_lowercase().contains("cooperative") => {
                script
                    .construct_claim(preimage, self.params(record, keys, destination, false))
                    .await?
            }
            Err(e) => return Err(e),
        };
        Ok(Self::signed(&tx))
    }

    async fn sign_refund(&self, record: &SwapRecord, keys: &Keypair) -> Result<SignedTx, Error> {
        let Some(address) = record.state.refund_address.clone() else {
            return Err(Error::Generic(
                "no refund address was recorded for this swap".into(),
            ));
        };
        let script = Self::script(record, keys)?;
        // Cooperative first, the maker co-signing so there is no timelock to
        // wait out; when it will not, the script path is always available once
        // the timelock has matured, so fall back to it rather than leave the
        // lockup behind.
        let tx = match script
            .construct_refund(self.params(record, keys, address.clone(), true))
            .await
        {
            Ok(tx) => tx,
            Err(_) => {
                script
                    .construct_refund(self.params(record, keys, address, false))
                    .await?
            }
        };
        Ok(Self::signed(&tx))
    }

    async fn broadcast(&self, record: &SwapRecord, tx: &SignedTx) -> Result<(), Error> {
        let tx = BtcLikeTransaction::from_hex(Self::chain_kind(record), &tx.hex)?;
        self.chain.broadcast_tx(&tx).await.map(|_| ())
    }

    async fn tx_seen(&self, record: &SwapRecord, tx: &SignedTx) -> Result<bool, Error> {
        let missing = |chain: &str| Error::Generic(format!("no {chain} chain client configured"));
        let no_outputs = || Error::Generic(format!("{} has no outputs", tx.txid));
        // The chain knows a transaction exactly when it can report on its
        // first output, in the mempool or in a block.
        let status = match BtcLikeTransaction::from_hex(Self::chain_kind(record), &tx.hex)? {
            BtcLikeTransaction::Bitcoin(tx) => {
                let out = tx.output.first().ok_or_else(no_outputs)?;
                self.chain
                    .bitcoin_client()
                    .ok_or_else(|| missing("Bitcoin"))?
                    .get_output_status(
                        &bitcoin::OutPoint::new(tx.compute_txid(), 0),
                        &out.script_pubkey,
                    )
                    .await?
            }
            BtcLikeTransaction::Liquid(tx) => {
                let out = tx.output.first().ok_or_else(no_outputs)?;
                self.chain
                    .liquid_client()
                    .ok_or_else(|| missing("Liquid"))?
                    .get_output_status(&elements::OutPoint::new(tx.txid(), 0), &out.script_pubkey)
                    .await?
            }
        };
        Ok(status.is_some())
    }

    async fn lockup_tip_height(&self, record: &SwapRecord) -> Result<u32, Error> {
        let missing = |chain: &str| Error::Generic(format!("no {chain} chain client configured"));
        match Self::chain_kind(record) {
            ChainKind::Bitcoin(_) => {
                self.chain
                    .bitcoin_client()
                    .ok_or_else(|| missing("Bitcoin"))?
                    .get_tip_height()
                    .await
            }
            ChainKind::Liquid(_) => {
                self.chain
                    .liquid_client()
                    .ok_or_else(|| missing("Liquid"))?
                    .get_tip_height()
                    .await
            }
        }
    }

    async fn tx_confirmed(&self, record: &SwapRecord, txid: &str) -> Result<bool, Error> {
        let missing = |chain: &str| Error::Generic(format!("no {chain} chain client configured"));
        let bad_txid = |e: &dyn fmt::Display| Error::Generic(format!("txid {txid}: {e}"));
        // The transaction's first output is in a block exactly when the
        // transaction is.
        let status = match Self::chain_kind(record) {
            ChainKind::Bitcoin(_) => {
                let client = self
                    .chain
                    .bitcoin_client()
                    .ok_or_else(|| missing("Bitcoin"))?;
                let txid: bitcoin::Txid = txid.parse().map_err(|e| bad_txid(&e))?;
                let tx = client.get_tx(txid).await?;
                let out = tx
                    .output
                    .first()
                    .ok_or_else(|| Error::Generic(format!("{txid} has no outputs")))?;
                client
                    .get_output_status(&bitcoin::OutPoint::new(txid, 0), &out.script_pubkey)
                    .await?
            }
            ChainKind::Liquid(_) => {
                let client = self
                    .chain
                    .liquid_client()
                    .ok_or_else(|| missing("Liquid"))?;
                let txid: elements::Txid = txid.parse().map_err(|e| bad_txid(&e))?;
                let tx = client.get_tx(txid).await?;
                let out = tx
                    .output
                    .first()
                    .ok_or_else(|| Error::Generic(format!("{txid} has no outputs")))?;
                client
                    .get_output_status(&elements::OutPoint::new(txid, 0), &out.script_pubkey)
                    .await?
            }
        };
        Ok(status.is_some_and(|s| s.block_height.is_some()))
    }

    async fn lockup_unspent(&self, record: &SwapRecord) -> Result<bool, Error> {
        let missing = |chain: &str| Error::Generic(format!("no {chain} chain client configured"));
        let address = match &record.state.response {
            CreateResponse::BoltzReverse(r) => r.lockup_address.as_str(),
            CreateResponse::BoltzSubmarine(r) => r.address.as_str(),
        };
        let bad_address =
            |e: &dyn fmt::Display| Error::Generic(format!("lockup address {address}: {e}"));
        match Self::chain_kind(record) {
            ChainKind::Bitcoin(_) => {
                // `verify_lockup` rebuilt this address from our own key
                // before the record was persisted.
                let address = address
                    .parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
                    .map_err(|e| bad_address(&e))?
                    .assume_checked();
                let client = self
                    .chain
                    .bitcoin_client()
                    .ok_or_else(|| missing("Bitcoin"))?;
                Ok(!client.get_address_utxos(&address).await?.is_empty())
            }
            ChainKind::Liquid(_) => {
                let address: elements::Address = address.parse().map_err(|e| bad_address(&e))?;
                let client = self
                    .chain
                    .liquid_client()
                    .ok_or_else(|| missing("Liquid"))?;
                Ok(!client.get_address_utxos(&address).await?.is_empty())
            }
        }
    }
}

/// Blocks a reverse swap's lockup must have left before its timeout when
/// `start` hands out the invoice: room for the lockup to confirm, and then
/// the claim path's own safety margin, which refuses a claim this close to
/// the maker's refund ([`crate::swaps::ClaimPolicy`]: 6 on Bitcoin, 60 on
/// Liquid, roughly an hour each). An invoice whose lockup could not be
/// claimed in time is never one to pay.
fn reverse_timeout_headroom(chain: ChainKind) -> u32 {
    use crate::swaps::ClaimPolicy;
    match chain {
        ChainKind::Bitcoin(_) => 2 * ClaimPolicy::BITCOIN_SAFETY_MARGIN_BLOCKS,
        ChainKind::Liquid(_) => 2 * ClaimPolicy::LIQUID_SAFETY_MARGIN_BLOCKS,
    }
}

// ---------------------------------------------------------------------------
// Pricing and validation

/// `ceil(amount * percent / 100)`, percent as the maker publishes it (e.g. 0.5).
fn percent_fee(amount: u64, percent: f64) -> u64 {
    let basis_points = (percent * 10_000.0).round() as u128;
    let scaled = amount as u128 * basis_points;
    scaled.div_ceil(1_000_000) as u64
}

fn miner_fee_total(fees: &PairMinerFees) -> u64 {
    fees.lockup + fees.claim
}

/// Parses `value` as an address (or BOLT11 invoice) of `asset`'s layer and
/// checks it belongs to `network`, so a malformed or wrong-network value is a
/// local error before the maker is contacted.
fn check_address(
    value: &str,
    asset: &Asset,
    network: Network,
    what: &str,
) -> Result<(), SwapError> {
    let layer = asset.layer();
    let invalid = |e: &dyn fmt::Display| {
        SwapError::new(
            SwapErrorCode::InvalidDestination,
            format!("{what} {value} is not a valid {layer:?} address: {e}"),
        )
    };
    let wrong_network = || {
        SwapError::new(
            SwapErrorCode::WrongNetworkAddress,
            format!("{what} {value} is not a {network:?} {layer:?} address"),
        )
    };
    let on_network = match layer {
        Layer::Arkade | Layer::Spark => return Ok(()),
        Layer::Bitcoin => value
            .parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
            .map_err(|e| invalid(&e))?
            .is_valid_for_network(network.into()),
        Layer::Liquid => {
            let address: elements::Address = value.parse().map_err(|e| invalid(&e))?;
            let expected: &'static elements::AddressParams = LiquidChain::from(network).into();
            *address.params == *expected
        }
        Layer::Lightning => {
            let invoice: lightning_invoice::Bolt11Invoice =
                value.trim().parse().map_err(|e| invalid(&e))?;
            invoice.currency() == lightning_currency(network)
        }
    };
    if on_network {
        Ok(())
    } else {
        Err(wrong_network())
    }
}

fn lightning_currency(network: Network) -> lightning_invoice::Currency {
    match network {
        Network::Mainnet => lightning_invoice::Currency::Bitcoin,
        Network::Testnet => lightning_invoice::Currency::BitcoinTestnet,
        Network::Signet => lightning_invoice::Currency::Signet,
        Network::Regtest => lightning_invoice::Currency::Regtest,
    }
}

/// What this layer reads off a BOLT11 invoice.
struct InvoiceTerms {
    /// Satoshis, when the invoice names an amount.
    amount_sat: Option<u64>,
    /// Unix seconds after which the invoice can no longer be paid.
    expires_at: u64,
}

fn decode_invoice(invoice: &str) -> Result<InvoiceTerms, SwapError> {
    let parsed: lightning_invoice::Bolt11Invoice = invoice.trim().parse().map_err(|e| {
        SwapError::new(
            SwapErrorCode::InvalidDestination,
            format!("invalid BOLT11: {e}"),
        )
    })?;
    Ok(InvoiceTerms {
        amount_sat: parsed.amount_milli_satoshis().map(|msat| msat / 1000),
        // `None` only when timestamp plus expiry overflows: never, in practice.
        expires_at: parsed.expires_at().map_or(u64::MAX, |at| at.as_secs()),
    })
}

/// Refuses an invoice whose expiry has passed, by the local clock: the maker
/// could not pay it, so a swap funded against it could only be refunded.
fn check_invoice_unexpired(terms: &InvoiceTerms) -> Result<(), SwapError> {
    let now = unix_now();
    if now >= terms.expires_at {
        return Err(SwapError::new(
            SwapErrorCode::InvoiceExpired,
            format!(
                "the invoice expired {} s ago; ask the payee for a new one",
                now - terms.expires_at
            ),
        ));
    }
    Ok(())
}

/// A swap id names a store entry — a file, for a file store — so a maker
/// cannot be allowed to pick anything but a plain token.
pub fn is_plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

struct Transition {
    phase: SwapPhase,
    funded_seen: bool,
}

/// Boltz statuses onto the shared machine. A submarine that expires or whose
/// invoice fails after we funded it is refundable; before funding there is
/// nothing to refund. A reverse swap never needs a refund from us — the
/// maker's hold invoice expires on its own — so every failure there is `Failed`.
fn map_status(protocol: Protocol, status: &str, state: &SwapState) -> Transition {
    let refundable = if protocol == Protocol::BoltzSubmarine && state.funded_seen {
        SwapPhase::Refundable
    } else {
        SwapPhase::Failed
    };
    let t = |phase, funded_seen| Transition { phase, funded_seen };
    match status {
        "swap.created" | "invoice.set" | "minerfee.paid" => t(SwapPhase::AwaitingFunding, false),
        "transaction.mempool" | "transaction.zeroconf.rejected" => t(SwapPhase::Funded, true),
        "transaction.confirmed" | "invoice.pending" => t(SwapPhase::Locked, true),
        "transaction.claim.pending" => t(SwapPhase::Claiming, true),
        "invoice.paid" | "invoice.settled" | "transaction.claimed" => t(SwapPhase::Settled, false),
        "transaction.refunded" => t(SwapPhase::Refunded, false),
        // Only a submarine swap has a lockup of ours to take back; a reverse
        // swap that fails is over, whatever the maker calls it.
        "transaction.lockupFailed" | "invoice.failedToPay" => {
            if protocol == Protocol::BoltzSubmarine {
                t(SwapPhase::Refundable, true)
            } else {
                t(SwapPhase::Failed, false)
            }
        }
        "swap.expired" | "invoice.expired" | "transaction.failed" | "invoice.error" => {
            t(refundable, false)
        }
        _ => t(
            if state.funded_seen {
                SwapPhase::Funded
            } else {
                SwapPhase::AwaitingFunding
            },
            false,
        ),
    }
}

/// Whether a failed refund attempt is one that waiting can fix: the timelock
/// has not matured yet, or the chain or the maker could not be reached. A
/// malformed address, a signing error or a rejected spend will fail the same
/// way every time, so retrying it only hides it.
fn spend_error_is_transient(error: &Error) -> bool {
    match error {
        Error::Locktime(_) | Error::IO(_) | Error::HTTP(_) => true,
        Error::HTTPStatusNotSuccess(status, _) => !status.is_client_error(),
        #[cfg(feature = "esplora")]
        Error::Esplora(_) => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// A swap in flight

/// The two spends this layer signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spend {
    Claim,
    Refund,
}

impl Spend {
    fn pending(self, record: &mut SwapRecord) -> &mut Option<SignedTx> {
        match self {
            Spend::Claim => &mut record.state.pending_claim,
            Spend::Refund => &mut record.state.pending_refund,
        }
    }
}

/// A handle over one record. Drive it with [`Swap::tick`] for one observation,
/// [`Swap::watch`] to follow it to the end, or [`Swap::settled`] to wait.
///
/// Every handle one [`KaleidoSwap`] gives out on the same id — from `start`,
/// `swap` or `resume` — shares one lock: a tick or refund waits for any other
/// in flight on that swap, then reloads the record from the store before it
/// acts, so it sees a claim or refund another handle already recorded.
/// Separate instances over one store do not share it.
pub struct Swap {
    env: Arc<Env>,
    lock: Arc<AsyncMutex<()>>,
    current: std::sync::Mutex<SwapRecord>,
    /// Phase changes since the last drain, so a watcher sees every step a
    /// single `tick` took, not only where it ended.
    transitions: std::sync::Mutex<Vec<SwapPhase>>,
    /// Whether the last refund attempt failed in a way another attempt can
    /// get past (an immature timelock, a chain or maker outage), as opposed
    /// to one that will fail the same way again.
    refund_retryable: std::sync::atomic::AtomicBool,
}

impl fmt::Debug for Swap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.current.lock().unwrap();
        f.debug_struct("Swap")
            .field("id", &r.id)
            .field("protocol", &r.protocol)
            .field("phase", &r.phase)
            .finish_non_exhaustive()
    }
}

impl Swap {
    fn new(env: Arc<Env>, record: SwapRecord) -> Self {
        Self {
            lock: env.swap_lock(&record.id),
            env,
            current: std::sync::Mutex::new(record),
            transitions: std::sync::Mutex::new(vec![]),
            refund_retryable: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The phase changes recorded since the previous call.
    pub fn drain_transitions(&self) -> Vec<SwapPhase> {
        std::mem::take(&mut *self.transitions.lock().unwrap())
    }

    pub fn id(&self) -> String {
        self.current.lock().unwrap().id.clone()
    }

    pub fn protocol(&self) -> Protocol {
        self.current.lock().unwrap().protocol
    }

    pub fn phase(&self) -> SwapPhase {
        self.current.lock().unwrap().phase
    }

    pub fn action(&self) -> SwapAction {
        self.current.lock().unwrap().action()
    }

    /// The record as this handle last saw it.
    pub fn record(&self) -> SwapRecord {
        self.current.lock().unwrap().clone()
    }

    async fn persist(&self, update: impl FnOnce(&mut SwapRecord)) -> Result<SwapRecord, SwapError> {
        let mut next = self.record();
        update(&mut next);
        next.updated_at = unix_now();
        self.env.store.put(&next).await.map_err(|e| SwapError {
            code: SwapErrorCode::StoreFailed,
            message: format!("could not persist swap {}: {}", next.id, e.message),
            swap_id: Some(next.id.clone()),
        })?;
        let mut current = self.current.lock().unwrap();
        if next.phase != current.phase {
            self.transitions.lock().unwrap().push(next.phase);
        }
        *current = next.clone();
        Ok(next)
    }

    /// Takes up what another handle on this swap persisted since this one
    /// last looked. Called with the swap's lock held; the store is the
    /// record of truth, since every change lands there first.
    async fn reload(&self) -> Result<(), SwapError> {
        let id = self.id();
        let Some(stored) = self.env.store.get(&id).await? else {
            return Ok(());
        };
        let mut current = self.current.lock().unwrap();
        if stored.phase != current.phase {
            self.transitions.lock().unwrap().push(stored.phase);
        }
        *current = stored;
        Ok(())
    }

    async fn keys(&self) -> Result<(Keypair, Preimage), SwapError> {
        let index = self.record().state.swap_index;
        let keys = self.env.signer.derive_key(index).await?;
        let preimage = self.env.signer.derive_preimage(index).await?;
        Ok((keys, preimage))
    }

    /// One observation of the maker plus whatever action it calls for.
    /// Returns once that action's broadcast is recorded; the maker's own
    /// progress past it shows up on the next tick.
    pub async fn tick(&self) -> Result<SwapRecord, SwapError> {
        let _held = self.lock.lock().await;
        self.reload().await?;
        self.tick_locked().await
    }

    async fn tick_locked(&self) -> Result<SwapRecord, SwapError> {
        let before = self.record();
        if before.phase.is_terminal() {
            return Ok(before);
        }
        let status = self
            .env
            .maker
            .swap_status(&before.id)
            .await
            .map_err(|e| SwapError::from_core(e, Some(&before.id)))?;
        let Transition {
            mut phase,
            mut funded_seen,
        } = map_status(before.protocol, &status, &before.state);
        let chain_err = |e: Error| {
            SwapError::for_swap(
                SwapErrorCode::ChainUnreachable,
                format!("could not read the chain: {e:?}"),
                &before.id,
            )
        };

        // A refund we broadcast ends the swap once it confirms. The maker's
        // status never says so for a submarine swap: it stays at whatever
        // made the swap refundable.
        if let Some(txid) = &before.state.refund_txid {
            if !matches!(phase, SwapPhase::Settled | SwapPhase::Refunded)
                && self
                    .env
                    .flows
                    .tx_confirmed(&before, txid)
                    .await
                    .map_err(chain_err)?
            {
                return self
                    .persist(|r| {
                        r.raw_status = status.clone();
                        r.phase = SwapPhase::Refunded;
                    })
                    .await;
            }
        }

        // An expired or failed submarine swap has something to refund when
        // its lockup holds funds, whether or not a poll saw them arrive. The
        // chain decides; an unreadable chain decides nothing yet.
        if before.protocol == Protocol::BoltzSubmarine
            && phase == SwapPhase::Failed
            && before.state.refund_txid.is_none()
            && self
                .env
                .flows
                .lockup_unspent(&before)
                .await
                .map_err(chain_err)?
        {
            phase = SwapPhase::Refundable;
            funded_seen = true;
        }

        // A claim or refund saved before a broadcast whose outcome was never
        // recorded — the process stopped in between — is finished before
        // anything else is decided: the chain may already have it, and a
        // second one signed now would spend the same lockup.
        if !matches!(phase, SwapPhase::Settled | SwapPhase::Refunded) {
            if let Some(tx) = before.state.pending_refund.clone() {
                return self.broadcast_saved(Spend::Refund, tx, Some(&status)).await;
            }
            if let Some(tx) = before.state.pending_claim.clone() {
                return self.broadcast_saved(Spend::Claim, tx, Some(&status)).await;
            }
        }

        // A claim or refund we already broadcast is not undone by the maker
        // still reporting the phase that triggered it.
        if before.state.claim_txid.is_some()
            && matches!(phase, SwapPhase::Locked | SwapPhase::Funded)
        {
            return self
                .persist(|r| {
                    r.raw_status = status.clone();
                    r.phase = SwapPhase::Claiming;
                })
                .await;
        }
        if before.state.refund_txid.is_some() && phase == SwapPhase::Refundable {
            return self
                .persist(|r| {
                    r.raw_status = status.clone();
                    r.phase = SwapPhase::Refunding;
                })
                .await;
        }

        self.persist(|r| {
            r.raw_status = status.clone();
            r.phase = phase;
            if funded_seen {
                r.state.funded_seen = true;
            }
        })
        .await?;

        match (phase, before.protocol) {
            (SwapPhase::Locked, Protocol::BoltzReverse) => self.claim_now().await,
            (SwapPhase::Refundable, _) => self.refund_now().await,
            _ => Ok(self.record()),
        }
    }

    async fn claim_now(&self) -> Result<SwapRecord, SwapError> {
        let (keys, preimage) = self.keys().await?;
        let record = self.record();
        let tx = self
            .env
            .flows
            .sign_claim(&record, &keys, &preimage)
            .await
            .map_err(|e| SwapError::from_core(e, Some(&record.id)))?;
        self.save_and_broadcast(Spend::Claim, tx).await
    }

    async fn refund_now(&self) -> Result<SwapRecord, SwapError> {
        let (keys, _) = self.keys().await?;
        let record = self.record();
        if record.state.refund_address.is_none() {
            return Err(SwapError::for_swap(
                SwapErrorCode::RefundAddressRequired,
                "no refund address was recorded for this swap",
                &record.id,
            ));
        }
        let tx = match self.env.flows.sign_refund(&record, &keys).await {
            Ok(tx) => tx,
            Err(e) => {
                self.refund_retryable.store(
                    spend_error_is_transient(&e),
                    std::sync::atomic::Ordering::Relaxed,
                );
                return Err(SwapError::from_core(e, Some(&record.id)));
            }
        };
        self.save_and_broadcast(Spend::Refund, tx).await
    }

    /// Saves a signed claim or refund, then broadcasts it: a broadcast that
    /// reaches the chain is never one the record has no trace of.
    async fn save_and_broadcast(&self, kind: Spend, tx: SignedTx) -> Result<SwapRecord, SwapError> {
        self.persist(|r| *kind.pending(r) = Some(tx.clone()))
            .await?;
        self.broadcast_saved(kind, tx, None).await
    }

    /// Broadcasts a saved claim or refund and records its txid once the
    /// chain has it: accepted now, or already there from a broadcast whose
    /// answer was lost. When the chain neither takes it nor has it, the saved
    /// transaction is dropped so the next attempt signs afresh, as it would
    /// have without one; when the chain cannot be read, it is kept.
    async fn broadcast_saved(
        &self,
        kind: Spend,
        tx: SignedTx,
        status: Option<&str>,
    ) -> Result<SwapRecord, SwapError> {
        let record = self.record();
        if let Err(error) = self.env.flows.broadcast(&record, &tx).await {
            let seen = self.env.flows.tx_seen(&record, &tx).await.map_err(|e| {
                SwapError::for_swap(
                    SwapErrorCode::ChainUnreachable,
                    format!(
                        "could not tell whether {} reached the chain: {e:?}",
                        tx.txid
                    ),
                    &record.id,
                )
            })?;
            if !seen {
                if kind == Spend::Refund {
                    self.refund_retryable.store(
                        spend_error_is_transient(&error),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                self.persist(|r| {
                    if let Some(status) = status {
                        r.raw_status = status.to_string();
                    }
                    *kind.pending(r) = None;
                })
                .await?;
                return Err(SwapError::from_core(error, Some(&record.id)));
            }
        }
        self.persist(|r| {
            if let Some(status) = status {
                r.raw_status = status.to_string();
            }
            *kind.pending(r) = None;
            match kind {
                Spend::Claim => {
                    r.phase = SwapPhase::Claiming;
                    r.state.claim_txid = Some(tx.txid.clone());
                }
                Spend::Refund => {
                    r.phase = SwapPhase::Refunding;
                    r.state.refund_txid = Some(tx.txid.clone());
                }
            }
        })
        .await
    }

    /// Unwinds a `Refundable` swap. The driver does this on its own; it is
    /// here for a caller who wants to decide.
    pub async fn refund(&self) -> Result<SwapRecord, SwapError> {
        let _held = self.lock.lock().await;
        self.reload().await?;
        let record = self.record();
        if record.state.refund_txid.is_some() {
            return Ok(record);
        }
        if let Some(tx) = record.state.pending_refund.clone() {
            return self.broadcast_saved(Spend::Refund, tx, None).await;
        }
        if record.phase != SwapPhase::Refundable {
            return Err(SwapError::for_swap(
                SwapErrorCode::SwapFailed,
                format!(
                    "swap {} is {}, not refundable",
                    record.id,
                    record.phase.as_str()
                ),
                &record.id,
            ));
        }
        self.refund_now().await
    }

    /// Follows the swap to a terminal phase, calling `on_phase` on every
    /// transition. Resolves with the record on `Settled`; any other end is
    /// an error. A maker or chain it cannot reach, or a refund that cannot
    /// be spent yet (its timelock has not matured and the maker will not
    /// co-sign), is waited out rather than ending the watch.
    pub async fn watch<F>(&self, mut on_phase: F) -> Result<SwapRecord, SwapError>
    where
        F: FnMut(SwapPhase) + Send,
    {
        self.drain_transitions();
        loop {
            let ticked = self.tick().await;
            // A tick can move the phase and then fail, as a refund that
            // cannot be spent yet does: report the move either way.
            for phase in self.drain_transitions() {
                on_phase(phase);
            }
            let record = match ticked {
                Ok(record) => record,
                Err(error) if self.keeps_watching(&error) => {
                    sleep(self.env.poll_interval).await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            match record.phase {
                SwapPhase::Settled => return Ok(record),
                SwapPhase::Refunded => {
                    return Err(SwapError::for_swap(
                        SwapErrorCode::SwapFailed,
                        format!("swap {} was refunded", record.id),
                        &record.id,
                    ))
                }
                SwapPhase::Failed => {
                    let code = if record.raw_status.contains("expired") {
                        SwapErrorCode::SwapExpired
                    } else {
                        SwapErrorCode::SwapFailed
                    };
                    return Err(SwapError::for_swap(
                        code,
                        format!("swap {} failed: {}", record.id, record.raw_status),
                        &record.id,
                    ));
                }
                _ => sleep(self.env.poll_interval).await,
            }
        }
    }

    /// Whether `error` is one more poll can get past.
    fn keeps_watching(&self, error: &SwapError) -> bool {
        matches!(
            error.code,
            SwapErrorCode::MakerUnreachable | SwapErrorCode::ChainUnreachable
        ) || (self.phase() == SwapPhase::Refundable
            && error.code == SwapErrorCode::SwapFailed
            && self
                .refund_retryable
                .load(std::sync::atomic::Ordering::Relaxed))
    }

    /// [`Swap::watch`] without a listener.
    pub async fn settled(&self) -> Result<SwapRecord, SwapError> {
        self.watch(|_| {}).await
    }
}

// ---------------------------------------------------------------------------
// The instance

pub struct ResumeReport {
    /// Still in flight after one pass; follow each with `settled()` or `watch()`.
    pub resumed: Vec<Swap>,
    pub settled: Vec<SwapRecord>,
    pub refunded: Vec<SwapRecord>,
    pub failed: Vec<SwapRecord>,
    /// Swaps this pass could not move — the maker did not know them, a claim
    /// was refused — each with its error. They stay in the store, and one of
    /// them never stops the others from being resumed.
    pub errors: Vec<ResumeFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeFailure {
    pub id: String,
    pub error: SwapError,
}

pub struct KaleidoSwap {
    env: Arc<Env>,
}

impl KaleidoSwap {
    /// The constructor. `maker_url` must be given on a network KaleidoSwap
    /// runs no maker on.
    pub fn new(options: KaleidoSwapOptions) -> Result<Self, SwapError> {
        let network = options.network;
        let base_url = match &options.maker_url {
            Some(url) => url.clone(),
            None => BoltzApiClientV2::default(network)
                .map_err(|e| {
                    SwapError::new(
                        SwapErrorCode::PairUnavailable,
                        format!("KaleidoSwap runs no maker on {network:?}; pass maker_url to name one ({e:?})"),
                    )
                })?
                .base_url()
                .to_string(),
        };
        let timeout = options.maker_timeout.unwrap_or(Duration::from_secs(30));
        let api = BoltzApiClientV2::new(base_url, Some(timeout));
        let maker_url = api.base_url().to_string();
        let defaults = default_chain_urls(network);
        let bitcoin_url = options
            .bitcoin_esplora_url
            .clone()
            .or_else(|| defaults.map(|d| d.0.to_string()));
        let liquid_url = options
            .liquid_esplora_url
            .clone()
            .or_else(|| defaults.map(|d| d.1.to_string()));
        let (Some(bitcoin_url), Some(liquid_url)) = (bitcoin_url, liquid_url) else {
            return Err(SwapError::new(
                SwapErrorCode::PairUnavailable,
                format!("no default chain access on {network:?}; pass bitcoin_esplora_url and liquid_esplora_url"),
            ));
        };
        let chain = ChainClient::new()
            .with_bitcoin(EsploraBitcoinClient::new(network.into(), &bitcoin_url, 30))
            .with_liquid(EsploraLiquidClient::new(network.into(), &liquid_url, 30));
        let api = Arc::new(api);
        let flows = DefaultFlows::new(api.clone(), chain, options.fee_sat_per_vb.unwrap_or(2.0));
        Ok(Self::with_seams(options, maker_url, api, Arc::new(flows)))
    }

    /// The constructor tests use: the maker and the chain steps are whatever
    /// the caller passes.
    pub fn with_seams(
        options: KaleidoSwapOptions,
        maker_url: String,
        maker: Arc<dyn MakerApi>,
        flows: Arc<dyn SwapFlows>,
    ) -> Self {
        Self {
            env: Arc::new(Env {
                network: options.network,
                maker_url,
                maker,
                store: options.store,
                signer: options.signer,
                flows,
                poll_interval: options.poll_interval.unwrap_or(Duration::from_secs(2)),
                quote_ttl: options.quote_ttl.unwrap_or(Duration::from_secs(600)),
                swap_locks: std::sync::Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn network(&self) -> Network {
        self.env.network
    }

    /// Prices `pay → receive`. The two sides decide the protocol; the
    /// amount has to be the kind that protocol is priced by.
    pub async fn quote(&self, request: QuoteRequest) -> Result<QuoteAnswer, SwapError> {
        let QuoteRequest {
            pay,
            receive,
            amount,
        } = request;
        let on_chain_btc = |a: &Asset| matches!(a, Asset::BtcOnchain | Asset::BtcLiquid);
        if pay == Asset::BtcLightning && on_chain_btc(&receive) {
            let (amount, fixes_receive) = match amount {
                Amount::Pay(amount) => (amount, false),
                Amount::Receive(amount) => (amount, true),
                Amount::Invoice(_) => {
                    return Err(SwapError::new(
                        SwapErrorCode::InvalidAmount,
                        format!("{pay} -> {receive} is priced by an amount: pass Amount::Pay or Amount::Receive, not an invoice"),
                    ))
                }
            };
            return self
                .quote_reverse(pay, receive, amount, fixes_receive)
                .await;
        }
        if on_chain_btc(&pay) && receive == Asset::BtcLightning {
            let Amount::Invoice(invoice) = amount else {
                return Err(SwapError::new(
                    SwapErrorCode::InvalidDestination,
                    format!("{pay} -> {receive} is priced against the invoice it pays: pass Amount::Invoice"),
                ));
            };
            return self.quote_submarine(pay, receive, invoice).await;
        }
        let message = unrouted(&pay, &receive);
        Ok(QuoteAnswer::Refusal(Refusal {
            from_asset: pay,
            to_asset: receive,
            reason: QuoteRefusalReason::PairUnavailable,
            limits: None,
            message,
        }))
    }

    fn expires_at(&self) -> u64 {
        unix_now() + self.env.quote_ttl.as_secs()
    }

    async fn quote_reverse(
        &self,
        from: Asset,
        to: Asset,
        amount: u64,
        fixes_receive: bool,
    ) -> Result<QuoteAnswer, SwapError> {
        let pairs = self
            .env
            .maker
            .reverse_pairs()
            .await
            .map_err(|e| SwapError::from_core(e, None))?;
        let Some(card): Option<ReversePair> =
            pairs.get(from.boltz_symbol()?, to.boltz_symbol()?).cloned()
        else {
            return Ok(QuoteAnswer::Refusal(Refusal {
                message: format!("the maker does not advertise {from} -> {to}"),
                from_asset: from,
                to_asset: to,
                reason: QuoteRefusalReason::PairUnavailable,
                limits: None,
            }));
        };
        let percent = card.fees.percentage;
        if let Some(refusal) = unusable_percentage(&from, &to, percent) {
            return Ok(QuoteAnswer::Refusal(refusal));
        }
        let miner = miner_fee_total(&card.fees.miner_fees);
        let from_amount = if fixes_receive {
            // Invert `to = from - ceil(from * pct) - miner` closely enough; the
            // maker's response carries the exact payout and the record keeps
            // that one.
            let gross = (amount + miner) as u128 * 1_000_000;
            let divisor = 1_000_000u128 - (percent * 10_000.0).round() as u128;
            gross.div_ceil(divisor) as u64
        } else {
            amount
        };
        let maker_fee = percent_fee(from_amount, percent);
        let to_amount = from_amount.saturating_sub(maker_fee + miner);
        let limits = Limits {
            min: card.limits.minimal,
            max: card.limits.maximal,
        };
        if let Some(refusal) = out_of_limits(&from, &to, from_amount, limits) {
            return Ok(QuoteAnswer::Refusal(refusal));
        }
        Ok(QuoteAnswer::Quote(Quote {
            protocol: Protocol::BoltzReverse,
            from_asset: from,
            to_asset: to,
            from_amount,
            to_amount,
            fees: Fees {
                maker: maker_fee,
                miner,
            },
            limits,
            expires_at: self.expires_at(),
            binding: QuoteBinding {
                pair_hash: card.hash,
                invoice: None,
                zero_conf_limit: 0,
            },
        }))
    }

    async fn quote_submarine(
        &self,
        from: Asset,
        to: Asset,
        invoice: String,
    ) -> Result<QuoteAnswer, SwapError> {
        check_address(&invoice, &to, self.env.network, "invoice")?;
        let terms = decode_invoice(&invoice)?;
        check_invoice_unexpired(&terms)?;
        let Some(to_amount) = terms.amount_sat else {
            return Err(SwapError::new(
                SwapErrorCode::InvalidAmount,
                "the invoice carries no amount; a submarine swap needs one",
            ));
        };
        let pairs = self
            .env
            .maker
            .submarine_pairs()
            .await
            .map_err(|e| SwapError::from_core(e, None))?;
        let Some(card): Option<SubmarinePair> =
            pairs.get(from.boltz_symbol()?, to.boltz_symbol()?).cloned()
        else {
            return Ok(QuoteAnswer::Refusal(Refusal {
                message: format!("the maker does not advertise {from} -> {to}"),
                from_asset: from,
                to_asset: to,
                reason: QuoteRefusalReason::PairUnavailable,
                limits: None,
            }));
        };
        let percent = card.fees.percentage;
        if let Some(refusal) = unusable_percentage(&from, &to, percent) {
            return Ok(QuoteAnswer::Refusal(refusal));
        }
        let miner = card.fees.miner_fees;
        let maker_fee = percent_fee(to_amount, percent);
        let from_amount = to_amount + maker_fee + miner;
        let limits = Limits {
            min: card.limits.minimal,
            max: card.limits.maximal,
        };
        if let Some(refusal) = out_of_limits(&from, &to, to_amount, limits) {
            return Ok(QuoteAnswer::Refusal(refusal));
        }
        Ok(QuoteAnswer::Quote(Quote {
            protocol: Protocol::BoltzSubmarine,
            from_asset: from,
            to_asset: to,
            from_amount,
            to_amount,
            fees: Fees {
                maker: maker_fee,
                miner,
            },
            limits,
            // A quote never outlives the invoice it was priced against.
            expires_at: self.expires_at().min(terms.expires_at),
            binding: QuoteBinding {
                pair_hash: card.hash,
                invoice: Some(invoice),
                zero_conf_limit: card.limits.maximal_zero_conf,
            },
        }))
    }

    /// Creates the swap against `quote`, verifies the lockup against our own
    /// key and, for a reverse swap, that its timeout leaves room to confirm
    /// and claim, persists the record, then returns. Nothing is funded or
    /// paid here — that is the caller's step, via [`Swap::action`].
    pub async fn start(&self, quote: &Quote, options: StartOptions) -> Result<Swap, SwapError> {
        if unix_now() > quote.expires_at {
            return Err(SwapError::new(
                SwapErrorCode::QuoteExpired,
                "the quote has expired; ask for a new one",
            ));
        }
        let reverse = quote.protocol == Protocol::BoltzReverse;
        if !matches!(
            quote.protocol,
            Protocol::BoltzReverse | Protocol::BoltzSubmarine
        ) {
            return Err(SwapError::new(
                SwapErrorCode::PairUnavailable,
                format!("{:?} is not started by this layer", quote.protocol),
            ));
        }
        let destination = match (&quote.binding.invoice, &options.destination) {
            (Some(invoice), Some(given)) if given != invoice => {
                return Err(SwapError::new(
                    SwapErrorCode::InvalidDestination,
                    "destination differs from the invoice the quote was priced against",
                ))
            }
            (Some(invoice), _) => invoice.clone(),
            (None, Some(given)) => given.clone(),
            (None, None) => {
                return Err(SwapError::new(
                    SwapErrorCode::InvalidDestination,
                    "destination is required",
                ))
            }
        };
        check_address(
            &destination,
            &quote.to_asset,
            self.env.network,
            "destination",
        )?;
        if !reverse {
            // The quote is the caller's value: the invoice it carries is
            // checked again here rather than trusted to its `expires_at`.
            check_invoice_unexpired(&decode_invoice(&destination)?)?;
            let Some(refund) = &options.refund_address else {
                return Err(SwapError::new(
                    SwapErrorCode::RefundAddressRequired,
                    format!(
                        "a swap from {} needs a refund_address on its chain",
                        quote.from_asset
                    ),
                ));
            };
            check_address(
                refund,
                &quote.from_asset,
                self.env.network,
                "refund_address",
            )?;
        }

        let (from_symbol, to_symbol) = (
            quote.from_asset.boltz_symbol()?,
            quote.to_asset.boltz_symbol()?,
        );

        let swap_index = self.env.store.next_index().await?;
        let keys = self.env.signer.derive_key(swap_index).await?;
        let preimage = self.env.signer.derive_preimage(swap_index).await?;
        let pubkey = bitcoin::PublicKey::new(keys.public_key());

        let (id, response) = if reverse {
            // The terms the quote was priced on must still be the maker's.
            let pairs = self
                .env
                .maker
                .reverse_pairs()
                .await
                .map_err(|e| SwapError::from_core(e, None))?;
            let card = pairs
                .get(from_symbol, to_symbol)
                .filter(|card| card.hash == quote.binding.pair_hash)
                .cloned()
                .ok_or_else(|| {
                    SwapError::new(
                        SwapErrorCode::QuoteExpired,
                        "the pair terms changed since the quote; ask for a new one",
                    )
                })?;
            let request = CreateReverseRequest {
                from: from_symbol.to_string(),
                to: to_symbol.to_string(),
                claim_public_key: pubkey,
                invoice: None,
                invoice_amount: Some(quote.from_amount),
                preimage_hash: Some(preimage.sha256),
                pair_hash: Some(quote.binding.pair_hash.clone()),
                description: None,
                description_hash: None,
                address: None,
                address_signature: None,
                referral_id: None,
                webhook: None,
            };
            let r = self
                .env
                .maker
                .create_reverse(request.clone())
                .await
                .map_err(|e| SwapError::from_core(e, None))?;
            // The invoice must carry the amount we asked for, and the payout
            // must be at least what the accepted terms promise.
            r.validate_amounts(&request, Some(&card)).map_err(|e| {
                SwapError::for_swap(
                    SwapErrorCode::LockupMismatch,
                    format!("the maker's response does not match the request: {e:?}"),
                    &r.id,
                )
            })?;
            if r.onchain_amount < quote.to_amount {
                return Err(SwapError::for_swap(
                    SwapErrorCode::LockupMismatch,
                    format!(
                        "the maker would pay out {} but the quote promised {}",
                        r.onchain_amount, quote.to_amount
                    ),
                    &r.id,
                ));
            }
            (r.id.clone(), CreateResponse::BoltzReverse(r))
        } else {
            let r = self
                .env
                .maker
                .create_submarine(&CreateSubmarineRequest {
                    from: from_symbol.to_string(),
                    to: to_symbol.to_string(),
                    invoice: destination.clone(),
                    refund_public_key: pubkey,
                    pair_hash: Some(quote.binding.pair_hash.clone()),
                    referral_id: None,
                    webhook: None,
                })
                .await
                .map_err(|e| SwapError::from_core(e, None))?;
            // We fund exactly what the quote said, never more.
            if r.expected_amount > quote.from_amount {
                return Err(SwapError::for_swap(
                    SwapErrorCode::LockupMismatch,
                    format!(
                        "the maker asks for {} but the quote said {}",
                        r.expected_amount, quote.from_amount
                    ),
                    &r.id,
                ));
            }
            (r.id.clone(), CreateResponse::BoltzSubmarine(r))
        };
        if !is_plain_id(&id) {
            return Err(SwapError::new(
                SwapErrorCode::MakerRejected,
                format!("the maker returned an unusable swap id: {id}"),
            ));
        }

        let now = unix_now();
        let record = SwapRecord {
            id: id.clone(),
            protocol: quote.protocol,
            network: self.env.network,
            maker_url: self.env.maker_url.clone(),
            from_asset: quote.from_asset.clone(),
            to_asset: quote.to_asset.clone(),
            phase: SwapPhase::Created,
            raw_status: "swap.created".to_string(),
            created_at: now,
            updated_at: now,
            state: SwapState {
                swap_index,
                response,
                destination,
                refund_address: options.refund_address,
                claim_txid: None,
                refund_txid: None,
                pending_claim: None,
                pending_refund: None,
                funded_seen: false,
                zero_conf_limit: (!reverse).then_some(quote.binding.zero_conf_limit),
            },
        };

        // A tree we could not claim or refund has to fail here, before
        // anything is funded — not after. Checked on the record exactly as
        // the store will hold it, so a restart rebuilds the same tree.
        self.env
            .flows
            .verify_lockup(&record, &keys, &preimage)
            .map_err(|e| {
                SwapError::for_swap(
                    SwapErrorCode::LockupMismatch,
                    format!("the maker's lockup does not match our key: {e:?}"),
                    &id,
                )
            })?;

        // A reverse swap hands out an invoice to pay; its lockup has to leave
        // time to confirm and be claimed before the maker can take it back.
        // Measured against our own chain client, never the maker's heights.
        if let CreateResponse::BoltzReverse(r) = &record.state.response {
            let chain = DefaultFlows::chain_kind(&record);
            let tip = self
                .env
                .flows
                .lockup_tip_height(&record)
                .await
                .map_err(|e| {
                    SwapError::for_swap(
                        SwapErrorCode::ChainUnreachable,
                        format!(
                            "could not read the {chain:?} tip to check the lockup timeout: {e:?}"
                        ),
                        &id,
                    )
                })?;
            let headroom = reverse_timeout_headroom(chain);
            r.validate_timeout_headroom(tip, headroom).map_err(|_| {
                SwapError::for_swap(
                    SwapErrorCode::SwapExpired,
                    format!(
                        "the maker's lockup times out at block {}, {} blocks after the tip at {tip}; \
                         paying needs at least {headroom} to confirm and claim, so the invoice was not handed out",
                        r.timeout_block_height,
                        r.timeout_block_height.saturating_sub(tip),
                    ),
                    &id,
                )
            })?;
        }

        // swap_auth rides in the response, issued once. This write is the one
        // that keeps it.
        let swap = Swap::new(self.env.clone(), record);
        swap.persist(|_| {}).await?;
        swap.persist(|r| r.phase = SwapPhase::AwaitingFunding)
            .await?;
        Ok(swap)
    }

    /// A swap this instance or an earlier one started, by id.
    pub async fn swap(&self, id: &str) -> Result<Swap, SwapError> {
        match self.env.store.get(id).await? {
            Some(record) => Ok(Swap::new(self.env.clone(), record)),
            None => Err(SwapError::for_swap(
                SwapErrorCode::SwapNotFound,
                format!("no swap {id} in the store"),
                id,
            )),
        }
    }

    /// Picks every non-terminal record out of the store, asks the maker where
    /// each stands, and moves it one step: claims what is claimable, refunds
    /// what is refundable, reports the rest. Call it at startup, before
    /// anything else. It does not wait for funding or confirmations — the
    /// `resumed` handles do, when followed.
    pub async fn resume(&self) -> Result<ResumeReport, SwapError> {
        let mut report = ResumeReport {
            resumed: vec![],
            settled: vec![],
            refunded: vec![],
            failed: vec![],
            errors: vec![],
        };
        for record in self.env.store.list().await? {
            if record.phase.is_terminal() {
                continue;
            }
            let id = record.id.clone();
            let swap = Swap::new(self.env.clone(), record);
            let after = match swap.tick().await {
                Ok(after) => after,
                Err(error) => {
                    report.errors.push(ResumeFailure { id, error });
                    continue;
                }
            };
            match after.phase {
                SwapPhase::Settled => report.settled.push(after),
                SwapPhase::Refunded => report.refunded.push(after),
                SwapPhase::Failed => report.failed.push(after),
                _ => report.resumed.push(swap),
            }
        }
        Ok(report)
    }
}

/// Why `pay → receive` is not one this layer routes, and where to go
/// instead. Every route the engine reaches is named, so the refusal is a
/// pointer rather than a dead end.
fn unrouted(pay: &Asset, receive: &Asset) -> String {
    let either = |f: fn(&Asset) -> bool| f(pay) || f(receive);
    if either(|a| a.layer() == Layer::Spark) {
        format!("{pay} -> {receive}: this SDK does not settle on Spark; pay or receive the Lightning side from a Spark wallet instead")
    } else if either(|a| matches!(a, Asset::RgbLightning { .. })) {
        format!("{pay} -> {receive}: RGB over Lightning is served by the KaleidoSwap market API through an RGB Lightning node, not by this SDK")
    } else if either(|a| matches!(a, Asset::RgbOnchain { .. })) {
        format!("{pay} -> {receive} is an RGB swap, which this layer does not route yet; use the rgb or atomic module with an rgb-lib wallet")
    } else if either(|a| a.layer() == Layer::Arkade) {
        format!("{pay} -> {receive} rides the Arkade RFQ corridor, which this layer does not route yet; use the corridor module")
    } else if either(|a| *a == Asset::UsdtLiquid) {
        format!("{pay} -> {receive} needs a caller-funded Liquid spend, which this layer does not route yet; use SwapScript::prepare_liquid_claim")
    } else if pay.layer() != Layer::Lightning
        && receive.layer() != Layer::Lightning
        && pay.layer() != receive.layer()
    {
        format!("{pay} -> {receive} is a chain swap, which this layer does not route yet; use BoltzApiClientV2::post_chain_req")
    } else {
        format!("{pay} -> {receive} is not a route the maker serves")
    }
}

/// A pair card is the maker's input; a fee percentage outside `0..100`
/// cannot be priced and must not reach the arithmetic.
fn unusable_percentage(from: &Asset, to: &Asset, percent: f64) -> Option<Refusal> {
    if percent.is_finite() && (0.0..100.0).contains(&percent) {
        return None;
    }
    Some(Refusal {
        from_asset: from.clone(),
        to_asset: to.clone(),
        reason: QuoteRefusalReason::PairUnavailable,
        limits: None,
        message: format!("the pair card for {from} -> {to} carries an unusable fee percentage"),
    })
}

fn out_of_limits(from: &Asset, to: &Asset, amount: u64, limits: Limits) -> Option<Refusal> {
    let reason = if amount < limits.min {
        QuoteRefusalReason::AmountBelowMin
    } else if amount > limits.max {
        QuoteRefusalReason::AmountAboveMax
    } else {
        return None;
    };
    Some(Refusal {
        from_asset: from.clone(),
        to_asset: to.clone(),
        reason,
        limits: Some(limits),
        message: match reason {
            QuoteRefusalReason::AmountBelowMin => {
                format!("{amount} is below the pair minimum {}", limits.min)
            }
            _ => format!("{amount} is above the pair maximum {}", limits.max),
        },
    })
}

// ---------------------------------------------------------------------------
// Tests: the maker answers from a script of statuses, the chain steps are
// recorded, nothing touches a network.

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use std::sync::Mutex;

    // Hold invoices the signet maker issued (50 000 sat) and a regtest one
    // from lightning-invoice; only amount and prefix matter here.
    const INVOICE_SIGNET_50K: &str = "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e";
    const INVOICE_REGTEST_100K: &str = "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysyt5fzeueg687mkcum4we2pm452clxqdy9l4extcqs84ek4nte3s56s6pmwxhmx4zx3d4k6xsftlphw8ed60xerfchczpw8w8avdn92qpftvz36";

    /// A signet invoice for 50 000 sat, signed now, that expires
    /// `expires_in` seconds from now (in the past when negative).
    fn invoice_expiring_in(expires_in: i64) -> String {
        use bitcoin::hashes::{sha256, Hash};
        let now = unix_now() as i64;
        // Backdate the timestamp rather than shorten the expiry, so an
        // expired invoice still carries a real one.
        let issued = (now + expires_in - 3600).max(0) as u64;
        let key = SecretKey::from_slice(&[0x11; 32]).unwrap();
        lightning_invoice::InvoiceBuilder::new(lightning_invoice::Currency::Signet)
            .description("test".into())
            .payment_hash(sha256::Hash::hash(&issued.to_be_bytes()))
            .payment_secret(lightning_invoice::PaymentSecret([7; 32]))
            .duration_since_epoch(Duration::from_secs(issued))
            .min_final_cltv_expiry_delta(144)
            .amount_milli_satoshis(50_000_000)
            .expiry_time(Duration::from_secs(3600))
            .build_signed(|hash| Secp256k1::new().sign_ecdsa_recoverable(hash, &key))
            .unwrap()
            .to_string()
    }

    fn fresh_invoice() -> String {
        invoice_expiring_in(3600)
    }

    fn reverse_card() -> ReversePair {
        serde_json::from_value(serde_json::json!({
            "hash": "rev-hash", "rate": 1.0,
            "limits": {"minimal": 10_000, "maximal": 1_000_000},
            "fees": {"percentage": 0.5, "minerFees": {"lockup": 200, "claim": 100}},
        }))
        .unwrap()
    }

    fn submarine_card() -> SubmarinePair {
        serde_json::from_value(serde_json::json!({
            "hash": "sub-hash", "rate": 1.0,
            "limits": {"minimal": 10_000, "maximal": 1_000_000, "maximalZeroConf": 100_000},
            "fees": {"percentage": 0.1, "minerFees": 150},
        }))
        .unwrap()
    }

    #[derive(Default)]
    struct FakeMaker {
        statuses: Mutex<Vec<String>>,
        created: Mutex<Vec<serde_json::Value>>,
        reverse_id: Mutex<String>,
        /// Sats the reverse payout falls short of the quoted one.
        reverse_shortfall: Mutex<u64>,
        /// Overrides the submarine lockup amount the maker asks for.
        submarine_expected: Mutex<Option<u64>>,
        /// Overrides the hash the reverse pair card carries.
        reverse_card_hash: Mutex<Option<String>>,
        /// Overrides the fee percentage the reverse pair card carries.
        reverse_percentage: Mutex<Option<f64>>,
        /// Swap ids whose status the maker cannot give.
        unknown_ids: Mutex<Vec<String>>,
        /// Whether the submarine create response accepts zero-conf.
        submarine_zero_conf: Mutex<bool>,
        /// Overrides the BIP21 the submarine create response carries.
        submarine_bip21: Mutex<Option<String>>,
    }

    impl FakeMaker {
        fn script(&self, statuses: &[&str]) {
            *self.statuses.lock().unwrap() = statuses.iter().map(|s| s.to_string()).collect();
        }
    }

    #[macros::async_trait]
    impl MakerApi for FakeMaker {
        async fn reverse_pairs(&self) -> Result<GetReversePairsResponse, Error> {
            let mut card = reverse_card();
            if let Some(hash) = self.reverse_card_hash.lock().unwrap().clone() {
                card.hash = hash;
            }
            if let Some(percentage) = *self.reverse_percentage.lock().unwrap() {
                card.fees.percentage = percentage;
            }
            Ok(serde_json::from_value(serde_json::json!({
                "BTC": {"L-BTC": card.clone(), "BTC": card}
            }))
            .unwrap())
        }
        async fn submarine_pairs(&self) -> Result<GetSubmarinePairsResponse, Error> {
            Ok(serde_json::from_value(serde_json::json!({
                "BTC": {"BTC": submarine_card()}, "L-BTC": {"BTC": submarine_card()}
            }))
            .unwrap())
        }
        async fn create_reverse(
            &self,
            req: CreateReverseRequest,
        ) -> Result<CreateReverseResponse, Error> {
            self.created
                .lock()
                .unwrap()
                .push(serde_json::to_value(&req).unwrap());
            let id = self.reverse_id.lock().unwrap().clone();
            Ok(serde_json::from_value(serde_json::json!({
                "id": if id.is_empty() { "rev-1".to_string() } else { id },
                "invoice": INVOICE_SIGNET_50K,
                "swapTree": {
                    "claimLeaf": {"output": "00", "version": 196},
                    "refundLeaf": {"output": "01", "version": 196},
                },
                "lockupAddress": "tlq1lockup",
                "refundPublicKey":
                    "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
                "timeoutBlockHeight": 200,
                "onchainAmount": req.invoice_amount.unwrap() - 550 - *self.reverse_shortfall.lock().unwrap(),
                "swapAuth": "secret-auth",
            }))
            .unwrap())
        }
        async fn create_submarine(
            &self,
            req: &CreateSubmarineRequest,
        ) -> Result<CreateSubmarineResponse, Error> {
            self.created
                .lock()
                .unwrap()
                .push(serde_json::to_value(req).unwrap());
            let bip21 = self
                .submarine_bip21
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "bitcoin:tb1qlockup?amount=0.000502".into());
            Ok(serde_json::from_value(serde_json::json!({
                "acceptZeroConf": *self.submarine_zero_conf.lock().unwrap(),
                "address": "tb1qlockup",
                "bip21": bip21,
                "claimPublicKey":
                    "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
                "expectedAmount": self.submarine_expected.lock().unwrap().unwrap_or(50_200),
                "id": "sub-1",
                "swapTree": {
                    "claimLeaf": {"output": "00", "version": 196},
                    "refundLeaf": {"output": "01", "version": 196},
                },
                "timeoutBlockHeight": 200,
                "swapAuth": "secret-auth",
            }))
            .unwrap())
        }
        async fn swap_status(&self, id: &str) -> Result<String, Error> {
            if self.unknown_ids.lock().unwrap().iter().any(|u| u == id) {
                return Err(Error::Generic(format!("the maker does not know {id}")));
            }
            let mut statuses = self.statuses.lock().unwrap();
            if statuses.is_empty() {
                return Err(Error::Generic("script exhausted".into()));
            }
            let status = if statuses.len() > 1 {
                statuses.remove(0)
            } else {
                statuses[0].clone()
            };
            // `!down` scripts one poll the maker does not answer.
            if status == "!down" {
                return Err(Error::HTTPStatusNotSuccess(
                    reqwest::StatusCode::SERVICE_UNAVAILABLE,
                    serde_json::json!({}),
                ));
            }
            Ok(status)
        }
    }

    #[derive(Default)]
    struct FakeFlows {
        verify: Mutex<Vec<(String, String)>>,
        claim: Mutex<Vec<(String, String)>>,
        refund: Mutex<Vec<(String, String)>>,
        reject_lockup: bool,
        /// The lockup chain's tip. The fake lockups time out at block 200.
        tip: u32,
        chain_down: bool,
        /// The refund reads as confirmed from this check on (1-based);
        /// `None` never.
        refund_confirms_on: Option<u32>,
        confirm_checks: Mutex<u32>,
        /// Whether an unspent output pays the lockup address.
        lockup_funded: bool,
        /// Refund attempts that fail, as an immature timelock does, before
        /// one goes through.
        refund_failures: Mutex<u32>,
        /// Make those failures permanent: a refund that fails the same way
        /// every time.
        refund_failures_permanent: bool,
        /// Txids broadcast and accepted.
        broadcasts: Mutex<Vec<String>>,
        /// Broadcasts refused before one is accepted.
        broadcast_failures: Mutex<u32>,
        /// Txids the chain has, in the mempool or a block.
        on_chain: Mutex<Vec<String>>,
        /// When set, each broadcast records whether this store already held
        /// the transaction being broadcast.
        observe: Mutex<Option<Arc<MemoryStore>>>,
        saved_before_broadcast: Mutex<Vec<bool>>,
    }

    fn pubkey_hex(keys: &Keypair) -> String {
        keys.public_key().to_string()
    }

    /// Pending once, then ready: lets two futures polled together
    /// interleave at the chain step, as a real broadcast would.
    struct YieldOnce(bool);

    impl std::future::Future for YieldOnce {
        type Output = ();
        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<()> {
            if self.0 {
                return std::task::Poll::Ready(());
            }
            self.0 = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }

    #[macros::async_trait]
    impl SwapFlows for FakeFlows {
        fn verify_lockup(
            &self,
            record: &SwapRecord,
            keys: &Keypair,
            _preimage: &Preimage,
        ) -> Result<(), Error> {
            self.verify
                .lock()
                .unwrap()
                .push((record.id.clone(), pubkey_hex(keys)));
            if self.reject_lockup {
                return Err(Error::Generic("tree mismatch".into()));
            }
            Ok(())
        }
        async fn sign_claim(
            &self,
            record: &SwapRecord,
            keys: &Keypair,
            _preimage: &Preimage,
        ) -> Result<SignedTx, Error> {
            YieldOnce(false).await;
            self.claim
                .lock()
                .unwrap()
                .push((record.id.clone(), pubkey_hex(keys)));
            Ok(SignedTx {
                txid: "claim-txid".into(),
                hex: "claim-hex".into(),
            })
        }
        async fn sign_refund(
            &self,
            record: &SwapRecord,
            keys: &Keypair,
        ) -> Result<SignedTx, Error> {
            YieldOnce(false).await;
            {
                let mut failures = self.refund_failures.lock().unwrap();
                if *failures > 0 {
                    *failures -= 1;
                    return Err(if self.refund_failures_permanent {
                        Error::Address("the refund address cannot be spent to".into())
                    } else {
                        Error::Locktime("non-final: the timelock has not matured".into())
                    });
                }
            }
            self.refund
                .lock()
                .unwrap()
                .push((record.id.clone(), pubkey_hex(keys)));
            Ok(SignedTx {
                txid: "refund-txid".into(),
                hex: "refund-hex".into(),
            })
        }
        async fn broadcast(&self, record: &SwapRecord, tx: &SignedTx) -> Result<(), Error> {
            let observe = self.observe.lock().unwrap().clone();
            if let Some(store) = observe {
                let stored = store.get(&record.id).await.unwrap().unwrap();
                let saved = [stored.state.pending_claim, stored.state.pending_refund]
                    .contains(&Some(tx.clone()));
                self.saved_before_broadcast.lock().unwrap().push(saved);
            }
            {
                let mut failures = self.broadcast_failures.lock().unwrap();
                if *failures > 0 {
                    *failures -= 1;
                    return Err(Error::Generic("broadcast refused".into()));
                }
            }
            self.broadcasts.lock().unwrap().push(tx.txid.clone());
            self.on_chain.lock().unwrap().push(tx.txid.clone());
            Ok(())
        }
        async fn tx_seen(&self, _record: &SwapRecord, tx: &SignedTx) -> Result<bool, Error> {
            if self.chain_down {
                return Err(Error::Generic("connection refused".into()));
            }
            Ok(self.on_chain.lock().unwrap().contains(&tx.txid))
        }
        async fn lockup_tip_height(&self, _record: &SwapRecord) -> Result<u32, Error> {
            if self.chain_down {
                return Err(Error::Generic("connection refused".into()));
            }
            Ok(self.tip)
        }
        async fn tx_confirmed(&self, _record: &SwapRecord, _txid: &str) -> Result<bool, Error> {
            if self.chain_down {
                return Err(Error::Generic("connection refused".into()));
            }
            let mut checks = self.confirm_checks.lock().unwrap();
            *checks += 1;
            Ok(self.refund_confirms_on.is_some_and(|n| *checks >= n))
        }
        async fn lockup_unspent(&self, _record: &SwapRecord) -> Result<bool, Error> {
            if self.chain_down {
                return Err(Error::Generic("connection refused".into()));
            }
            Ok(self.lockup_funded)
        }
    }

    struct FakeSigner;

    fn key_for(index: u64) -> Keypair {
        let mut bytes = [0u8; 32];
        bytes[31] = (index + 1) as u8;
        Keypair::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&bytes).unwrap())
    }

    #[macros::async_trait]
    impl SwapSigner for FakeSigner {
        async fn derive_key(&self, index: u64) -> Result<Keypair, SwapError> {
            Ok(key_for(index))
        }
        async fn derive_preimage(&self, index: u64) -> Result<Preimage, SwapError> {
            Ok(Preimage::from_swap_key(&key_for(index)))
        }
    }

    struct Harness {
        kaleido: KaleidoSwap,
        maker: Arc<FakeMaker>,
        flows: Arc<FakeFlows>,
        store: Arc<MemoryStore>,
    }

    fn harness_with(store: Arc<MemoryStore>, flows: FakeFlows) -> Harness {
        let maker = Arc::new(FakeMaker::default());
        let flows = Arc::new(flows);
        let kaleido = KaleidoSwap::with_seams(
            KaleidoSwapOptions {
                network: Network::Signet,
                store: store.clone(),
                signer: Arc::new(FakeSigner),
                maker_url: None,
                bitcoin_esplora_url: None,
                liquid_esplora_url: None,
                poll_interval: Some(Duration::from_millis(1)),
                fee_sat_per_vb: None,
                quote_ttl: None,
                maker_timeout: None,
            },
            "https://maker.example/v2".into(),
            maker.clone(),
            flows.clone(),
        );
        Harness {
            kaleido,
            maker,
            flows,
            store,
        }
    }

    fn harness() -> Harness {
        harness_with(Arc::new(MemoryStore::new()), FakeFlows::default())
    }

    fn reverse_request(amount: u64) -> QuoteRequest {
        QuoteRequest {
            pay: Asset::BtcLightning,
            receive: Asset::BtcLiquid,
            amount: Amount::Pay(amount),
        }
    }

    fn submarine_request(invoice: &str) -> QuoteRequest {
        QuoteRequest {
            pay: Asset::BtcOnchain,
            receive: Asset::BtcLightning,
            amount: Amount::Invoice(invoice.into()),
        }
    }

    async fn reverse_quote(h: &Harness) -> Quote {
        h.kaleido
            .quote(reverse_request(50_000))
            .await
            .unwrap()
            .quote()
            .unwrap()
    }

    #[macros::async_test_all]
    async fn reverse_quote_prices_from_the_card_and_binds_its_hash() {
        let q = reverse_quote(&harness()).await;
        assert_eq!(q.protocol, Protocol::BoltzReverse);
        assert_eq!(q.from_amount, 50_000);
        assert_eq!(
            q.fees,
            Fees {
                maker: 250,
                miner: 300
            }
        );
        assert_eq!(q.to_amount, 49_450);
        assert_eq!(q.binding.pair_hash, "rev-hash");
    }

    #[macros::async_test_all]
    async fn a_receive_amount_inverts_the_fee_formula() {
        let h = harness();
        let q = h
            .kaleido
            .quote(QuoteRequest {
                amount: Amount::Receive(49_450),
                ..reverse_request(0)
            })
            .await
            .unwrap()
            .quote()
            .unwrap();
        assert!(q.to_amount >= 49_450, "{}", q.to_amount);
        assert!(q.from_amount - 50_000 <= 1, "{}", q.from_amount);
    }

    #[macros::async_test_all]
    async fn refusals_are_values() {
        let h = harness();
        let below = h
            .kaleido
            .quote(reverse_request(1))
            .await
            .unwrap()
            .quote()
            .unwrap_err();
        assert_eq!(below.reason, QuoteRefusalReason::AmountBelowMin);
        assert_eq!(
            below.limits,
            Some(Limits {
                min: 10_000,
                max: 1_000_000
            })
        );

        // Every route the engine reaches but this layer does not is refused
        // with a pointer to where it lives.
        let rgb = "rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd".to_string();
        for (pay, receive, says) in [
            (Asset::BtcArkade, Asset::BtcLightning, "Arkade"),
            (Asset::BtcLightning, Asset::UsdtLiquid, "Liquid spend"),
            (Asset::BtcOnchain, Asset::BtcLiquid, "chain swap"),
            (
                Asset::BtcLightning,
                Asset::RgbOnchain {
                    contract_id: rgb.clone(),
                },
                "RGB swap",
            ),
            (
                Asset::BtcOnchain,
                Asset::RgbOnchain {
                    contract_id: rgb.clone(),
                },
                "RGB swap",
            ),
            (
                Asset::BtcLightning,
                Asset::RgbLightning { contract_id: rgb },
                "RGB over Lightning",
            ),
            (Asset::BtcSpark, Asset::BtcLiquid, "Spark"),
            (Asset::BtcLightning, Asset::BtcLightning, "not a route"),
        ] {
            let refusal = h
                .kaleido
                .quote(QuoteRequest {
                    pay: pay.clone(),
                    receive: receive.clone(),
                    amount: Amount::Pay(50_000),
                })
                .await
                .unwrap()
                .quote()
                .unwrap_err();
            assert_eq!(refusal.reason, QuoteRefusalReason::PairUnavailable);
            assert_eq!((refusal.from_asset, refusal.to_asset), (pay, receive));
            assert!(refusal.message.contains(says), "{}", refusal.message);
        }
    }

    #[macros::async_test_all]
    async fn the_amount_must_be_the_kind_the_route_is_priced_by() {
        let h = harness();
        let invoice_from_lightning = h
            .kaleido
            .quote(QuoteRequest {
                amount: Amount::Invoice(INVOICE_SIGNET_50K.into()),
                ..reverse_request(0)
            })
            .await
            .unwrap_err();
        assert_eq!(invoice_from_lightning.code, SwapErrorCode::InvalidAmount);

        let amount_to_lightning = h
            .kaleido
            .quote(QuoteRequest {
                amount: Amount::Receive(50_000),
                ..submarine_request("")
            })
            .await
            .unwrap_err();
        assert_eq!(amount_to_lightning.code, SwapErrorCode::InvalidDestination);
    }

    #[macros::async_test_all]
    async fn an_expired_invoice_is_refused_locally_and_bounds_the_quote() {
        let h = harness();
        let err = h
            .kaleido
            .quote(submarine_request(&invoice_expiring_in(-60)))
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::InvoiceExpired, "{}", err.message);

        // A quote lasts no longer than its invoice.
        let q = h
            .kaleido
            .quote(submarine_request(&invoice_expiring_in(120)))
            .await
            .unwrap()
            .quote()
            .unwrap();
        assert!(q.expires_at <= unix_now() + 120, "{}", q.expires_at);

        // `start` reads the invoice itself, not only the quote's expiry.
        let stale = Quote {
            binding: QuoteBinding {
                invoice: Some(invoice_expiring_in(-60)),
                ..q.binding.clone()
            },
            ..q
        };
        let err = h
            .kaleido
            .start(
                &stale,
                StartOptions {
                    refund_address: Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::InvoiceExpired, "{}", err.message);
        assert!(
            h.maker.created.lock().unwrap().is_empty(),
            "nothing reached the maker"
        );
        assert_eq!(
            serde_json::to_value(SwapErrorCode::InvoiceExpired).unwrap(),
            "INVOICE_EXPIRED"
        );
    }

    #[macros::async_test_all]
    async fn submarine_is_priced_against_the_invoice_and_checks_its_network() {
        let h = harness();
        let invoice = fresh_invoice();
        let q = h
            .kaleido
            .quote(submarine_request(&invoice))
            .await
            .unwrap()
            .quote()
            .unwrap();
        assert_eq!(q.protocol, Protocol::BoltzSubmarine);
        assert_eq!(q.to_amount, 50_000);
        assert_eq!(q.from_amount, 50_000 + 50 + 150);
        assert_eq!(q.binding.invoice.as_deref(), Some(invoice.as_str()));

        let wrong = h
            .kaleido
            .quote(submarine_request(INVOICE_REGTEST_100K))
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);
    }

    #[macros::async_test_all]
    async fn start_validates_verifies_and_persists_before_returning() {
        let h = harness();
        let q = reverse_quote(&h).await;

        let wrong = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("ex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j20sf6s5".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);
        // A right-looking prefix is not an address.
        let malformed = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1qnotanaddress".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(malformed.code, SwapErrorCode::InvalidDestination);
        let none = h
            .kaleido
            .start(&q, StartOptions::default())
            .await
            .unwrap_err();
        assert_eq!(none.code, SwapErrorCode::InvalidDestination);
        assert!(
            h.maker.created.lock().unwrap().is_empty(),
            "nothing reached the maker"
        );

        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(swap.id(), "rev-1");
        assert_eq!(swap.phase(), SwapPhase::AwaitingFunding);
        assert_eq!(swap.action().invoice.as_deref(), Some(INVOICE_SIGNET_50K));
        assert_eq!(
            *h.flows.verify.lock().unwrap(),
            vec![("rev-1".to_string(), pubkey_hex(&key_for(0)))]
        );

        let req = h.maker.created.lock().unwrap()[0].clone();
        assert_eq!(req["from"], "BTC");
        assert_eq!(req["to"], "L-BTC");
        assert_eq!(req["invoiceAmount"], 50_000);
        assert_eq!(req["pairHash"], "rev-hash");
        assert_eq!(req["claimPublicKey"], pubkey_hex(&key_for(0)));

        let record = h.store.get("rev-1").await.unwrap().unwrap();
        assert_eq!(record.phase, SwapPhase::AwaitingFunding);
        assert_eq!(record.state.swap_index, 0);
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["state"]["response"]["swapAuth"], "secret-auth");
        assert_eq!(json["state"]["response"]["onchainAmount"], 49_450);
        assert_eq!(json["network"], "signet");
        // The record round-trips through JSON unchanged.
        let back: SwapRecord = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json);
    }

    #[macros::async_test_all]
    async fn a_lockup_that_does_not_match_our_key_never_reaches_the_store() {
        let h = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                reject_lockup: true,
                ..Default::default()
            },
        );
        let q = reverse_quote(&h).await;
        let err = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch);
        assert!(h.store.get("rev-1").await.unwrap().is_none());
    }

    #[macros::async_test_all]
    async fn a_reverse_lockup_too_close_to_its_timeout_is_never_handed_out() {
        // L-BTC needs 120 blocks of headroom; the fake lockup times out at 200.
        for (flows, code) in [
            (
                FakeFlows {
                    tip: 81,
                    ..Default::default()
                },
                SwapErrorCode::SwapExpired,
            ),
            (
                FakeFlows {
                    chain_down: true,
                    ..Default::default()
                },
                SwapErrorCode::ChainUnreachable,
            ),
        ] {
            let store = Arc::new(MemoryStore::new());
            let h = harness_with(store.clone(), flows);
            let q = reverse_quote(&h).await;
            let err = h
                .kaleido
                .start(
                    &q,
                    StartOptions {
                        destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(err.code, code, "{}", err.message);
            assert!(err.swap_id.is_some());
            assert!(store.list().await.unwrap().is_empty(), "nothing persisted");
        }

        // Exactly enough headroom is not enough; one block more is.
        let at_edge = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                tip: 80,
                ..Default::default()
            },
        );
        let q = reverse_quote(&at_edge).await;
        let err = at_edge
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapExpired);
        let roomy = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                tip: 79,
                ..Default::default()
            },
        );
        let q = reverse_quote(&roomy).await;
        roomy
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    #[macros::async_test_all]
    async fn each_swap_takes_a_fresh_index_and_expired_quotes_are_refused() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let dest = || StartOptions {
            destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
            ..Default::default()
        };
        h.kaleido.start(&q, dest()).await.unwrap();
        *h.maker.reverse_id.lock().unwrap() = "rev-2".into();
        h.kaleido.start(&q, dest()).await.unwrap();
        let keys: Vec<String> = h
            .maker
            .created
            .lock()
            .unwrap()
            .iter()
            .map(|c| c["claimPublicKey"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(keys, vec![pubkey_hex(&key_for(0)), pubkey_hex(&key_for(1))]);

        let expired = Quote { expires_at: 0, ..q };
        let err = h.kaleido.start(&expired, dest()).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::QuoteExpired);
    }

    #[macros::async_test_all]
    async fn start_checks_the_create_response_against_the_quote() {
        let dest = || StartOptions {
            destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
            ..Default::default()
        };

        // A payout below what the accepted terms promise.
        let h = harness();
        let q = reverse_quote(&h).await;
        *h.maker.reverse_shortfall.lock().unwrap() = 1;
        let err = h.kaleido.start(&q, dest()).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch, "{err}");
        assert!(h.store.list().await.unwrap().is_empty());

        // An invoice for a different amount than was requested: the stub
        // always answers with a 50 000 sat invoice.
        let h = harness();
        let q = h
            .kaleido
            .quote(reverse_request(40_000))
            .await
            .unwrap()
            .quote()
            .unwrap();
        let err = h.kaleido.start(&q, dest()).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch, "{err}");
        assert!(h.store.list().await.unwrap().is_empty());

        // A pair whose terms moved since the quote is not created against.
        let h = harness();
        let q = reverse_quote(&h).await;
        *h.maker.reverse_card_hash.lock().unwrap() = Some("other-hash".into());
        let err = h.kaleido.start(&q, dest()).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::QuoteExpired);
        assert!(
            h.maker.created.lock().unwrap().is_empty(),
            "nothing was created"
        );

        // A submarine lockup larger than the quoted amount.
        let h = harness();
        let q = h
            .kaleido
            .quote(submarine_request(&fresh_invoice()))
            .await
            .unwrap()
            .quote()
            .unwrap();
        *h.maker.submarine_expected.lock().unwrap() = Some(q.from_amount + 1);
        let refund = StartOptions {
            refund_address: Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".into()),
            ..Default::default()
        };
        let err = h.kaleido.start(&q, refund).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch, "{err}");
        assert!(h.store.list().await.unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn a_failed_reverse_swap_is_failed_not_refundable() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        h.maker
            .script(&["transaction.mempool", "invoice.failedToPay"]);
        let mut seen = vec![];
        let err = swap.watch(|p| seen.push(p)).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed);
        assert_eq!(seen, vec![SwapPhase::Funded, SwapPhase::Failed]);
        assert!(h.flows.refund.lock().unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn one_swap_the_maker_cannot_report_does_not_stop_the_others() {
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        let q = reverse_quote(&first).await;
        let dest = || StartOptions {
            destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
            ..Default::default()
        };
        *first.maker.reverse_id.lock().unwrap() = "a-pruned".into();
        first.kaleido.start(&q, dest()).await.unwrap();
        *first.maker.reverse_id.lock().unwrap() = "b-claimable".into();
        first.kaleido.start(&q, dest()).await.unwrap();

        let second = harness_with(store.clone(), FakeFlows::default());
        second
            .maker
            .unknown_ids
            .lock()
            .unwrap()
            .push("a-pruned".into());
        second.maker.script(&["transaction.confirmed"]);
        let report = second.kaleido.resume().await.unwrap();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].id, "a-pruned");
        assert_eq!(report.resumed.len(), 1);
        assert_eq!(report.resumed[0].id(), "b-claimable");
        assert_eq!(
            second.flows.claim.lock().unwrap().len(),
            1,
            "the claimable swap was claimed"
        );
        assert!(
            store.get("a-pruned").await.unwrap().is_some(),
            "the unreadable one stays in the store"
        );
    }

    #[macros::async_test_all]
    async fn a_pair_card_with_an_unusable_percentage_is_refused_not_priced() {
        let h = harness();
        for percentage in [100.0, 250.0, -1.0] {
            *h.maker.reverse_percentage.lock().unwrap() = Some(percentage);
            let refusal = h
                .kaleido
                .quote(QuoteRequest {
                    amount: Amount::Receive(50_000),
                    ..reverse_request(0)
                })
                .await
                .unwrap()
                .quote()
                .unwrap_err();
            assert_eq!(
                refusal.reason,
                QuoteRefusalReason::PairUnavailable,
                "{percentage}"
            );
        }
    }

    #[macros::async_test_all]
    async fn a_maker_supplied_id_that_is_not_a_plain_token_is_refused() {
        let h = harness();
        let q = reverse_quote(&h).await;
        *h.maker.reverse_id.lock().unwrap() = "../x".into();
        let err = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::MakerRejected);
        assert!(h.store.list().await.unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn reverse_runs_to_settled_and_claims_once_on_confirmation() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        h.maker.script(&[
            "swap.created",
            "transaction.mempool",
            "transaction.confirmed",
            "transaction.confirmed", // still reported after our claim broadcast
            "invoice.settled",
        ]);
        let mut seen = vec![];
        let done = swap.watch(|p| seen.push(p)).await.unwrap();
        assert_eq!(
            seen,
            vec![
                SwapPhase::Funded,
                SwapPhase::Locked,
                SwapPhase::Claiming,
                SwapPhase::Settled
            ]
        );
        assert_eq!(h.flows.claim.lock().unwrap().len(), 1);
        assert_eq!(done.state.claim_txid.as_deref(), Some("claim-txid"));
        assert_eq!(done.raw_status, "invoice.settled");
        assert_eq!(
            h.store.get("rev-1").await.unwrap().unwrap().phase,
            SwapPhase::Settled
        );
    }

    #[macros::async_test_all]
    async fn settled_rejects_with_the_terminal_reason() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        h.maker.script(&["swap.created", "swap.expired"]);
        let err = swap.settled().await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapExpired);
        assert_eq!(err.swap_id.as_deref(), Some("rev-1"));
        assert!(
            h.flows.refund.lock().unwrap().is_empty(),
            "a reverse swap has nothing of ours to refund"
        );
    }

    #[macros::async_test_all]
    async fn submarine_funding_then_a_failed_payment_refunds_on_its_own() {
        let h = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                refund_confirms_on: Some(2),
                ..Default::default()
            },
        );
        let q = h
            .kaleido
            .quote(submarine_request(&fresh_invoice()))
            .await
            .unwrap()
            .quote()
            .unwrap();

        let none = h
            .kaleido
            .start(&q, StartOptions::default())
            .await
            .unwrap_err();
        assert_eq!(none.code, SwapErrorCode::RefundAddressRequired);
        let wrong = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);
        let malformed = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qnotanaddress".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(malformed.code, SwapErrorCode::InvalidDestination);

        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            swap.action(),
            SwapAction {
                funding_address: Some("tb1qlockup".into()),
                funding_amount: Some(50_200),
                bip21: Some("bitcoin:tb1qlockup?amount=0.000502".into()),
                zero_conf_limit: Some(0),
                invoice: None,
            }
        );
        assert_eq!(
            h.maker.created.lock().unwrap()[0]["invoice"],
            q.binding.invoice.clone().unwrap()
        );

        // A submarine maker never reports a refund: it stays at the status
        // that made the swap refundable. The refund ends the swap once the
        // chain client sees it confirmed.
        h.maker
            .script(&["transaction.mempool", "invoice.failedToPay"]);
        let mut seen = vec![];
        let err = swap.watch(|p| seen.push(p)).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed);
        assert_eq!(
            seen,
            vec![
                SwapPhase::Funded,
                SwapPhase::Refundable,
                SwapPhase::Refunding,
                SwapPhase::Refunded
            ]
        );
        assert_eq!(h.flows.refund.lock().unwrap().len(), 1);
        assert_eq!(swap.record().raw_status, "invoice.failedToPay");
        assert_eq!(
            h.kaleido.swap(&swap.id()).await.unwrap().phase(),
            SwapPhase::Refunded
        );
    }

    async fn started_submarine(h: &Harness) -> Swap {
        let q = h
            .kaleido
            .quote(submarine_request(&fresh_invoice()))
            .await
            .unwrap()
            .quote()
            .unwrap();
        h.kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
    }

    #[macros::async_test_all]
    async fn watch_waits_out_an_unreachable_maker_and_an_immature_refund() {
        let h = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                refund_confirms_on: Some(1),
                refund_failures: Mutex::new(1),
                ..Default::default()
            },
        );
        let swap = started_submarine(&h).await;
        h.maker.script(&[
            "!down",
            "transaction.mempool",
            "!down",
            "invoice.failedToPay",
        ]);
        let mut seen = vec![];
        let err = swap.watch(|p| seen.push(p)).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed, "{}", err.message);
        assert_eq!(
            seen,
            vec![
                SwapPhase::Funded,
                SwapPhase::Refundable,
                SwapPhase::Refunding,
                SwapPhase::Refunded
            ]
        );
        assert_eq!(h.flows.refund.lock().unwrap().len(), 1);

        // Anything else still ends the watch.
        let other = started_submarine(&h).await;
        h.maker.unknown_ids.lock().unwrap().push(other.id());
        let err = other.watch(|_| {}).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed);
    }

    #[macros::async_test_all]
    async fn watch_ends_on_a_refund_that_will_fail_every_time() {
        let h = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                refund_failures: Mutex::new(u32::MAX),
                refund_failures_permanent: true,
                ..Default::default()
            },
        );
        let swap = started_submarine(&h).await;
        h.maker
            .script(&["transaction.mempool", "invoice.failedToPay"]);
        let err = swap.watch(|_| {}).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed, "{}", err.message);
        assert_eq!(swap.phase(), SwapPhase::Refundable);
        assert_eq!(*h.flows.refund_failures.lock().unwrap(), u32::MAX - 1);
    }

    #[macros::async_test_all]
    async fn an_expired_submarine_whose_lockup_holds_funds_is_refunded() {
        // No poll ever saw the funding: the app was not running between
        // funding and expiry. The chain says the lockup holds funds.
        let h = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                lockup_funded: true,
                refund_confirms_on: Some(1),
                ..Default::default()
            },
        );
        let swap = started_submarine(&h).await;
        h.maker.script(&["swap.expired"]);
        let mut seen = vec![];
        swap.watch(|p| seen.push(p)).await.unwrap_err();
        assert_eq!(
            seen,
            vec![
                SwapPhase::Refundable,
                SwapPhase::Refunding,
                SwapPhase::Refunded
            ]
        );
        assert_eq!(h.flows.refund.lock().unwrap().len(), 1);

        // A chain it cannot read decides nothing: the swap is not failed.
        let blind = harness_with(
            Arc::new(MemoryStore::new()),
            FakeFlows {
                chain_down: true,
                ..Default::default()
            },
        );
        let swap = started_submarine(&blind).await;
        blind.maker.script(&["swap.expired"]);
        let err = swap.tick().await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::ChainUnreachable);
        assert_eq!(swap.phase(), SwapPhase::AwaitingFunding);
        assert!(blind.flows.refund.lock().unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn action_carries_the_quoted_zero_conf_limit_when_the_maker_accepts_zero_conf() {
        let h = harness();
        *h.maker.submarine_zero_conf.lock().unwrap() = true;
        let swap = started_submarine(&h).await;
        assert_eq!(swap.action().zero_conf_limit, Some(100_000));
        // The limit is part of the stored record, so a resumed handle has it.
        assert_eq!(
            h.kaleido
                .swap(&swap.id())
                .await
                .unwrap()
                .action()
                .zero_conf_limit,
            Some(100_000)
        );

        // A record written before the limit was kept still reads.
        let mut json = serde_json::to_value(swap.record()).unwrap();
        json["state"]
            .as_object_mut()
            .unwrap()
            .remove("zeroConfLimit");
        let old: SwapRecord = serde_json::from_value(json).unwrap();
        assert_eq!(old.action().zero_conf_limit, None);
    }

    #[macros::async_test_all]
    async fn action_passes_on_a_bip21_only_when_it_names_the_verified_funding() {
        for (bip21, passed) in [
            ("bitcoin:tb1qlockup?amount=0.000502", true),
            ("bitcoin:tb1qlockup?amount=0.000502&label=swap", true),
            ("BITCOIN:tb1qlockup?amount=.000502", true),
            ("bitcoin:tb1qlockup?amount=0.0005021", false),
            ("bitcoin:tb1qlockup?amount=0.000503", false),
            ("bitcoin:tb1qlockup", false),
            ("bitcoin:tb1qother?amount=0.000502", false),
            ("bitcoin:tb1qlockup?amount=0.000502&lightning=lntbs1", false),
            ("bitcoin:tb1qlockup?amount=0.000502&req-x=1", false),
            ("liquidtestnet:tb1qlockup?amount=0.000502", false),
            ("bitcoin:tb1qlockup?amount=5.02e-4", false),
            ("", false),
        ] {
            let h = harness();
            *h.maker.submarine_bip21.lock().unwrap() = Some(bip21.into());
            let swap = started_submarine(&h).await;
            assert_eq!(swap.action().bip21.is_some(), passed, "{bip21}");
        }
        // On Liquid the asset, when named, has to be L-BTC.
        let CreateResponse::BoltzSubmarine(mut r) =
            started_submarine(&harness()).await.record().state.response
        else {
            unreachable!()
        };
        let lbtc = LiquidChain::from(Network::Signet).bitcoin().to_string();
        r.bip21 = format!("liquidtestnet:tb1qlockup?amount=0.000502&assetid={lbtc}");
        assert!(checked_bip21(&Asset::BtcLiquid, Network::Signet, &r).is_some());
        r.bip21 = format!(
            "liquidtestnet:tb1qlockup?amount=0.000502&assetid={}",
            "ab".repeat(32)
        );
        assert!(checked_bip21(&Asset::BtcLiquid, Network::Signet, &r).is_none());

        assert_eq!(btc_decimal_to_sat("1"), Some(100_000_000));
        assert_eq!(btc_decimal_to_sat("0.00000001"), Some(1));
        assert_eq!(btc_decimal_to_sat("."), None);
        assert_eq!(btc_decimal_to_sat("-1"), None);
    }

    #[macros::async_test_all]
    async fn submarine_expiry_before_funding_is_failed_not_refundable() {
        let h = harness();
        let q = h
            .kaleido
            .quote(submarine_request(&fresh_invoice()))
            .await
            .unwrap()
            .quote()
            .unwrap();
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        h.maker.script(&["swap.created", "swap.expired"]);
        let mut seen = vec![];
        swap.watch(|p| seen.push(p)).await.unwrap_err();
        assert_eq!(seen, vec![SwapPhase::Failed]);
        assert!(h.flows.refund.lock().unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn resume_acts_once_and_reports_the_rest() {
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        let q = reverse_quote(&first).await;
        first
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // A new process over the same store: the previous one never saw the lockup.
        let second = harness_with(store.clone(), FakeFlows::default());
        second.maker.script(&["transaction.confirmed"]);
        let report = second.kaleido.resume().await.unwrap();
        assert_eq!(report.resumed.len(), 1);
        assert_eq!(report.resumed[0].phase(), SwapPhase::Claiming);
        assert_eq!(second.flows.claim.lock().unwrap().len(), 1);
        assert_eq!(
            store
                .get("rev-1")
                .await
                .unwrap()
                .unwrap()
                .state
                .claim_txid
                .as_deref(),
            Some("claim-txid")
        );

        // A third pass sees the claim already broadcast and does not repeat it.
        let third = harness_with(store.clone(), FakeFlows::default());
        third
            .maker
            .script(&["transaction.confirmed", "invoice.settled"]);
        let again = third.kaleido.resume().await.unwrap();
        assert!(third.flows.claim.lock().unwrap().is_empty());
        assert_eq!(again.resumed.len(), 1);
        assert_eq!(
            third.kaleido.swap("rev-1").await.unwrap().phase(),
            SwapPhase::Claiming
        );
        let done = again.resumed[0].settled().await.unwrap();
        assert_eq!(done.phase, SwapPhase::Settled);

        let quiet = third.kaleido.resume().await.unwrap();
        assert!(quiet.resumed.is_empty() && quiet.settled.is_empty() && quiet.errors.is_empty());
    }

    #[macros::async_test_all]
    async fn handles_on_one_swap_take_turns_and_claim_once() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let first = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let second = h.kaleido.swap(&first.id()).await.unwrap();
        h.maker.script(&["transaction.confirmed"]);
        let (a, b) = futures_util::future::join(first.tick(), second.tick()).await;
        a.unwrap();
        b.unwrap();
        assert_eq!(h.flows.claim.lock().unwrap().len(), 1, "claimed once");
        assert_eq!(first.phase(), SwapPhase::Claiming);
        assert_eq!(second.phase(), SwapPhase::Claiming);
        assert_eq!(
            second.record().state.claim_txid.as_deref(),
            Some("claim-txid")
        );

        // A handle that sat out the claim takes it up on its next tick and
        // reports the move to its own watcher.
        let third = h.kaleido.swap(&first.id()).await.unwrap();
        first.tick().await.unwrap();
        third.tick().await.unwrap();
        assert_eq!(h.flows.claim.lock().unwrap().len(), 1);
    }

    #[macros::async_test_all]
    async fn handles_on_one_swap_refund_once() {
        let h = harness();
        let first = started_submarine(&h).await;
        let second = h.kaleido.swap(&first.id()).await.unwrap();
        h.maker.script(&["invoice.failedToPay"]);
        let (a, b) = futures_util::future::join(first.tick(), second.tick()).await;
        a.unwrap();
        b.unwrap();
        assert_eq!(h.flows.refund.lock().unwrap().len(), 1, "refunded once");
        assert_eq!(second.phase(), SwapPhase::Refunding);
        // An explicit refund on a handle that has not ticked since does not
        // sign another.
        let stale = h.kaleido.swap(&first.id()).await.unwrap();
        let (c, d) = futures_util::future::join(first.refund(), stale.refund()).await;
        c.unwrap();
        d.unwrap();
        assert_eq!(h.flows.refund.lock().unwrap().len(), 1);

        // A lock lives as long as the last handle on its swap.
        drop((first, second, stale));
        let held = |id: &str| h.kaleido.env.swap_locks.lock().unwrap()[id].strong_count();
        assert_eq!(held("sub-1"), 0);
        let again = h.kaleido.swap("sub-1").await.unwrap();
        assert_eq!(held("sub-1"), 1);
        drop(again);
    }

    async fn started_reverse(h: &Harness) -> Swap {
        let q = reverse_quote(h).await;
        h.kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
    }

    /// Writes `id` back to `store` as a process that saved a signed claim
    /// or refund and stopped before recording its broadcast would have.
    async fn save_unrecorded(store: &MemoryStore, id: &str, phase: SwapPhase, kind: Spend) {
        let mut record = store.get(id).await.unwrap().unwrap();
        record.phase = phase;
        let tx = match kind {
            Spend::Claim => "claim",
            Spend::Refund => "refund",
        };
        *kind.pending(&mut record) = Some(SignedTx {
            txid: format!("{tx}-txid"),
            hex: format!("{tx}-hex"),
        });
        store.put(&record).await.unwrap();
    }

    #[macros::async_test_all]
    async fn a_claim_is_saved_before_it_is_broadcast() {
        let h = harness();
        *h.flows.observe.lock().unwrap() = Some(h.store.clone());
        let swap = started_reverse(&h).await;
        h.maker.script(&["transaction.confirmed"]);
        let record = swap.tick().await.unwrap();
        assert_eq!(*h.flows.saved_before_broadcast.lock().unwrap(), vec![true]);
        assert_eq!(record.state.claim_txid.as_deref(), Some("claim-txid"));
        assert_eq!(record.state.pending_claim, None);
        let stored = serde_json::to_value(h.store.get("rev-1").await.unwrap().unwrap()).unwrap();
        assert!(stored["state"].get("pendingClaim").is_none(), "{stored}");
    }

    #[macros::async_test_all]
    async fn a_saved_claim_is_finished_on_the_next_tick_not_signed_again() {
        // The chain took the broadcast and already has it: rebroadcasting is
        // refused, and the chain's copy is what gets recorded.
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        started_reverse(&first).await;
        save_unrecorded(&store, "rev-1", SwapPhase::Locked, Spend::Claim).await;
        let second = harness_with(
            store.clone(),
            FakeFlows {
                broadcast_failures: Mutex::new(1),
                on_chain: Mutex::new(vec!["claim-txid".into()]),
                ..Default::default()
            },
        );
        second.maker.script(&["transaction.confirmed"]);
        let report = second.kaleido.resume().await.unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(
            second.flows.claim.lock().unwrap().is_empty(),
            "not re-signed"
        );
        let record = store.get("rev-1").await.unwrap().unwrap();
        assert_eq!(record.phase, SwapPhase::Claiming);
        assert_eq!(record.state.claim_txid.as_deref(), Some("claim-txid"));
        assert_eq!(record.state.pending_claim, None);

        // The broadcast never happened: the saved claim is broadcast as is.
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        started_reverse(&first).await;
        save_unrecorded(&store, "rev-1", SwapPhase::Locked, Spend::Claim).await;
        let second = harness_with(store.clone(), FakeFlows::default());
        second.maker.script(&["transaction.confirmed"]);
        second.kaleido.resume().await.unwrap();
        assert!(
            second.flows.claim.lock().unwrap().is_empty(),
            "not re-signed"
        );
        assert_eq!(*second.flows.broadcasts.lock().unwrap(), vec!["claim-txid"]);
        assert_eq!(
            store
                .get("rev-1")
                .await
                .unwrap()
                .unwrap()
                .state
                .claim_txid
                .as_deref(),
            Some("claim-txid")
        );
    }

    #[macros::async_test_all]
    async fn a_saved_claim_the_chain_refuses_and_lacks_is_dropped_and_signed_afresh() {
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        started_reverse(&first).await;
        save_unrecorded(&store, "rev-1", SwapPhase::Locked, Spend::Claim).await;
        let second = harness_with(
            store.clone(),
            FakeFlows {
                broadcast_failures: Mutex::new(1),
                ..Default::default()
            },
        );
        second.maker.script(&["transaction.confirmed"]);
        let swap = second.kaleido.swap("rev-1").await.unwrap();
        let err = swap.tick().await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapFailed, "{}", err.message);
        assert_eq!(swap.record().state.pending_claim, None);
        assert_eq!(swap.record().state.claim_txid, None);
        swap.tick().await.unwrap();
        assert_eq!(second.flows.claim.lock().unwrap().len(), 1, "signed afresh");
        assert_eq!(
            swap.record().state.claim_txid.as_deref(),
            Some("claim-txid")
        );

        // A chain it cannot read decides nothing: the saved claim is kept.
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        started_reverse(&first).await;
        save_unrecorded(&store, "rev-1", SwapPhase::Locked, Spend::Claim).await;
        let blind = harness_with(
            store.clone(),
            FakeFlows {
                broadcast_failures: Mutex::new(1),
                chain_down: true,
                ..Default::default()
            },
        );
        blind.maker.script(&["transaction.confirmed"]);
        let err = blind
            .kaleido
            .swap("rev-1")
            .await
            .unwrap()
            .tick()
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::ChainUnreachable);
        assert!(store
            .get("rev-1")
            .await
            .unwrap()
            .unwrap()
            .state
            .pending_claim
            .is_some());
        assert!(blind.flows.claim.lock().unwrap().is_empty());
    }

    #[macros::async_test_all]
    async fn a_saved_refund_is_finished_not_signed_again() {
        let store = Arc::new(MemoryStore::new());
        let first = harness_with(store.clone(), FakeFlows::default());
        let id = started_submarine(&first).await.id();
        save_unrecorded(&store, &id, SwapPhase::Refundable, Spend::Refund).await;

        // Asked for explicitly…
        let second = harness_with(store.clone(), FakeFlows::default());
        let record = second
            .kaleido
            .swap(&id)
            .await
            .unwrap()
            .refund()
            .await
            .unwrap();
        assert_eq!(record.phase, SwapPhase::Refunding);
        assert_eq!(record.state.refund_txid.as_deref(), Some("refund-txid"));
        assert!(
            second.flows.refund.lock().unwrap().is_empty(),
            "not re-signed"
        );

        // …or by the driver.
        save_unrecorded(&store, &id, SwapPhase::Refundable, Spend::Refund).await;
        let mut record = store.get(&id).await.unwrap().unwrap();
        record.state.refund_txid = None;
        store.put(&record).await.unwrap();
        let third = harness_with(store.clone(), FakeFlows::default());
        third.maker.script(&["invoice.failedToPay"]);
        let record = third.kaleido.swap(&id).await.unwrap().tick().await.unwrap();
        assert_eq!(record.phase, SwapPhase::Refunding);
        assert_eq!(record.raw_status, "invoice.failedToPay");
        assert!(
            third.flows.refund.lock().unwrap().is_empty(),
            "not re-signed"
        );
        assert_eq!(*third.flows.broadcasts.lock().unwrap(), vec!["refund-txid"]);
    }

    #[macros::async_test_all]
    async fn unknown_ids_are_coded() {
        let err = harness().kaleido.swap("nope").await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapNotFound);
    }

    #[macros::test_all]
    fn assets_round_trip_and_plain_ids_are_checked() {
        let rgb = "rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd";
        for (asset, wire, code) in [
            (Asset::BtcOnchain, "bitcoin:BTC".to_string(), "BTC_L1"),
            (Asset::BtcLightning, "lightning:BTC".into(), "BTC_LN"),
            (Asset::BtcLiquid, "liquid:BTC".into(), "BTC_LIQUID"),
            (Asset::UsdtLiquid, "liquid:USDT".into(), "LIQUID_ASSET"),
            (Asset::BtcArkade, "arkade:BTC".into(), "BTC_ARKADE"),
            (Asset::BtcSpark, "spark:BTC".into(), "BTC_SPARK"),
            (
                Asset::RgbOnchain {
                    contract_id: rgb.into(),
                },
                format!("bitcoin:{rgb}"),
                "RGB_L1",
            ),
            (
                Asset::RgbLightning {
                    contract_id: rgb.into(),
                },
                format!("lightning:{rgb}"),
                "RGB_LN",
            ),
            (
                Asset::SparkToken {
                    token_id: "btkn1qtoken".into(),
                },
                "spark:btkn1qtoken".into(),
                "SPARK_SPARK",
            ),
        ] {
            assert_eq!(wire.parse::<Asset>().unwrap(), asset);
            assert_eq!(serde_json::to_value(&asset).unwrap(), wire.as_str());
            assert_eq!(asset.code(), code);
        }
        for bad in [
            "nope",
            "bitcoin:ETH",
            "bitcoin:rgb:",
            "liquid:rgb:x",
            "spark:",
        ] {
            assert!(bad.parse::<Asset>().is_err(), "{bad}");
        }
        assert!(is_plain_id("01M64CAVBFK6T58RWS72GK6AHV"));
        for bad in ["../x", "a/b", "", &"x".repeat(200)] {
            assert!(!is_plain_id(bad), "{bad}");
        }
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[macros::async_test]
    async fn file_store_round_trips_records_and_never_repeats_an_index() {
        let dir = std::env::temp_dir().join(format!("kaleidoswap-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = FileStore::new(&dir);
        assert!(store.list().await.unwrap().is_empty());
        assert_eq!(store.next_index().await.unwrap(), 0);
        assert_eq!(store.next_index().await.unwrap(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.join("index")).unwrap().trim(),
            "2"
        );

        let h = harness_with(Arc::new(MemoryStore::new()), FakeFlows::default());
        let q = reverse_quote(&h).await;
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    destination: Some("tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let record = swap.record();
        store.put(&record).await.unwrap();
        let back = store.get("rev-1").await.unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(store.list().await.unwrap().len(), 1);
        assert!(store.get("missing").await.unwrap().is_none());
        for bad in ["../escape", "a/b", ".hidden"] {
            assert_eq!(
                store.get(bad).await.unwrap_err().code,
                SwapErrorCode::StoreFailed
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("rev-1.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // Concurrent writes of one record never share a temporary file, so the
        // record on disk is always a whole one.
        let store = Arc::new(store);
        let writers: Vec<_> = (0..16)
            .map(|_| {
                let (store, record) = (store.clone(), record.clone());
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        store
                            .write(
                                "rev-1.json",
                                &serde_json::to_string_pretty(&record).unwrap(),
                            )
                            .unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        assert!(
            store.get("rev-1").await.unwrap().is_some(),
            "the record is still whole"
        );
        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")
            })
            .count();
        assert_eq!(leftovers, 0, "no temporary file is left behind");

        // A second instance on the same directory continues the counter.
        assert_eq!(FileStore::new(&dir).next_index().await.unwrap(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[macros::async_test_all]
    async fn mnemonic_signer_derives_what_the_master_key_derives() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let master = SwapMasterKey::from_mnemonic(mnemonic, None, Network::Signet).unwrap();
        let signer = MnemonicSigner::from_swap_mnemonic(mnemonic, None, Network::Signet).unwrap();
        assert_eq!(
            signer.derive_key(7).await.unwrap().public_key(),
            master.derive_swapkey(7).unwrap().public_key()
        );
        assert_eq!(
            signer.derive_preimage(7).await.unwrap().sha256,
            Preimage::from_swap_key(&master.derive_swapkey(7).unwrap()).sha256
        );
    }
}
