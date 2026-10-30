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

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

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
use crate::swaps::{ChainClient, SwapScript, SwapTransactionParams, TransactionOptions};
use crate::util::fees::Fee;
use crate::util::secrets::{Preimage, SwapMasterKey};
use crate::util::{sleep, unix_now};

// ---------------------------------------------------------------------------
// Assets and amounts

/// The layer an asset lives on. `Arkade` is Bitcoin on Arkade (`ARKD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Bitcoin,
    Lightning,
    Liquid,
    Arkade,
}

/// `<layer>:<asset>`. Spelling the layer is what makes a wrong-network
/// destination a local error instead of an unrecoverable payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetId {
    #[serde(rename = "bitcoin:BTC")]
    BitcoinBtc,
    #[serde(rename = "lightning:BTC")]
    LightningBtc,
    #[serde(rename = "liquid:BTC")]
    LiquidBtc,
    #[serde(rename = "liquid:USDT")]
    LiquidUsdt,
    #[serde(rename = "arkade:BTC")]
    ArkadeBtc,
}

impl AssetId {
    pub const ALL: [AssetId; 5] = [
        AssetId::BitcoinBtc,
        AssetId::LightningBtc,
        AssetId::LiquidBtc,
        AssetId::LiquidUsdt,
        AssetId::ArkadeBtc,
    ];

    pub fn layer(self) -> Layer {
        match self {
            AssetId::BitcoinBtc => Layer::Bitcoin,
            AssetId::LightningBtc => Layer::Lightning,
            AssetId::LiquidBtc | AssetId::LiquidUsdt => Layer::Liquid,
            AssetId::ArkadeBtc => Layer::Arkade,
        }
    }

    /// The maker's catalogue symbol.
    pub fn symbol(self) -> &'static str {
        match self {
            AssetId::BitcoinBtc | AssetId::LightningBtc => "BTC",
            AssetId::LiquidBtc => "L-BTC",
            AssetId::LiquidUsdt => "L-USDT",
            AssetId::ArkadeBtc => "ARKD",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AssetId::BitcoinBtc => "bitcoin:BTC",
            AssetId::LightningBtc => "lightning:BTC",
            AssetId::LiquidBtc => "liquid:BTC",
            AssetId::LiquidUsdt => "liquid:USDT",
            AssetId::ArkadeBtc => "arkade:BTC",
        }
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AssetId {
    type Err = SwapError;
    fn from_str(s: &str) -> Result<Self, SwapError> {
        AssetId::ALL
            .into_iter()
            .find(|a| a.as_str() == s)
            .ok_or_else(|| {
                SwapError::new(
                    SwapErrorCode::PairUnavailable,
                    format!(
                        "unknown asset {s}; one of {}",
                        AssetId::ALL.map(|a| a.as_str()).join(", ")
                    ),
                )
            })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteSide {
    From,
    To,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    #[serde(rename = "from")]
    pub from_asset: AssetId,
    #[serde(rename = "to")]
    pub to_asset: AssetId,
    /// Which side `amount` fixes. `From` when absent.
    pub side: Option<QuoteSide>,
    /// Atomic units of the side's asset. Omit when `invoice` fixes it.
    pub amount: Option<u64>,
    /// Required when `to` is Lightning: the route is priced against the
    /// invoice the maker will pay, and its amount is the `to` amount.
    pub invoice: Option<String>,
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
    pub from_asset: AssetId,
    #[serde(rename = "to")]
    pub to_asset: AssetId,
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
    pub from_asset: AssetId,
    #[serde(rename = "to")]
    pub to_asset: AssetId,
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
    pub claim_txid: Option<String>,
    pub refund_txid: Option<String>,
    /// Whether a lockup from us was ever seen — decides `failed` vs
    /// `refundable` on expiry.
    #[serde(default)]
    pub funded_seen: bool,
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
    pub from_asset: AssetId,
    #[serde(rename = "to")]
    pub to_asset: AssetId,
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
    /// Zero means the maker waits for a confirmation before acting.
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
                zero_conf_limit: if r.accept_zero_conf { None } else { Some(0) },
                ..Default::default()
            },
        }
    }
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
    async fn claim(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<String, Error>;
    async fn refund(&self, record: &SwapRecord, keys: &Keypair) -> Result<String, Error>;
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
        let tmp = self.dir.join(format!("{name}.{}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
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
            Protocol::BoltzReverse => record.to_asset,
            _ => record.from_asset,
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
            Protocol::BoltzReverse => record.to_asset,
            _ => record.from_asset,
        };
        let currency = Some(match on_chain {
            AssetId::LiquidBtc => Currency::LBtc,
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

    async fn claim(
        &self,
        record: &SwapRecord,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<String, Error> {
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
        self.chain.broadcast_tx(&tx).await
    }

    async fn refund(&self, record: &SwapRecord, keys: &Keypair) -> Result<String, Error> {
        let Some(address) = record.state.refund_address.clone() else {
            return Err(Error::Generic(
                "no refund address was recorded for this swap".into(),
            ));
        };
        let script = Self::script(record, keys)?;
        let tx = script
            .construct_refund(self.params(record, keys, address, true))
            .await?;
        self.chain.broadcast_tx(&tx).await
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

/// A prefix check, not validation — it catches a mainnet address used on signet.
fn check_address(
    value: &str,
    asset: AssetId,
    network: Network,
    what: &str,
) -> Result<(), SwapError> {
    let layer = asset.layer();
    let prefixes: &[&str] = match (layer, network) {
        (Layer::Arkade, _) => return Ok(()),
        (Layer::Bitcoin, Network::Mainnet) => &["bc1", "1", "3"],
        (Layer::Bitcoin, Network::Testnet | Network::Signet) => &["tb1", "m", "n", "2"],
        (Layer::Bitcoin, Network::Regtest) => &["bcrt1"],
        (Layer::Liquid, Network::Mainnet) => &["lq1", "ex1", "VJL", "VTp"],
        (Layer::Liquid, Network::Testnet | Network::Signet) => &["tlq1", "tex1", "vjT", "vjU"],
        (Layer::Liquid, Network::Regtest) => &["el1", "ert1", "CTE", "AzP"],
        (Layer::Lightning, Network::Mainnet) => &["lnbc"],
        (Layer::Lightning, Network::Testnet) => &["lntb"],
        (Layer::Lightning, Network::Signet) => &["lntbs"],
        (Layer::Lightning, Network::Regtest) => &["lnbcrt"],
    };
    let matches = prefixes.iter().any(|prefix| match layer {
        Layer::Lightning => {
            let lower = value.to_lowercase();
            // `lntb` is testnet3; `lntbs` is signet. Keep them apart.
            lower.starts_with(prefix) && (*prefix != "lntb" || !lower.starts_with("lntbs"))
        }
        _ => value.starts_with(prefix),
    });
    if matches {
        Ok(())
    } else {
        Err(SwapError::new(
            SwapErrorCode::WrongNetworkAddress,
            format!(
                "{what} {value} does not look like a {network:?} {layer:?} {} (expected a prefix among {})",
                if layer == Layer::Lightning { "invoice" } else { "address" },
                prefixes.join(", ")
            ),
        ))
    }
}

/// The amount and payment hash a BOLT11 invoice carries.
fn decode_invoice(invoice: &str) -> Result<Option<u64>, SwapError> {
    let parsed: lightning_invoice::Bolt11Invoice = invoice.trim().parse().map_err(|e| {
        SwapError::new(
            SwapErrorCode::InvalidDestination,
            format!("invalid BOLT11: {e}"),
        )
    })?;
    Ok(parsed.amount_milli_satoshis().map(|msat| msat / 1000))
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
        "transaction.lockupFailed" | "invoice.failedToPay" => t(SwapPhase::Refundable, true),
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

// ---------------------------------------------------------------------------
// A swap in flight

/// A handle over one record. Drive it with [`Swap::tick`] for one observation,
/// [`Swap::watch`] to follow it to the end, or [`Swap::settled`] to wait.
pub struct Swap {
    env: Arc<Env>,
    current: std::sync::Mutex<SwapRecord>,
    /// Phase changes since the last drain, so a watcher sees every step a
    /// single `tick` took, not only where it ended.
    transitions: std::sync::Mutex<Vec<SwapPhase>>,
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
            env,
            current: std::sync::Mutex::new(record),
            transitions: std::sync::Mutex::new(vec![]),
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
        let Transition { phase, funded_seen } = map_status(before.protocol, &status, &before.state);

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
        let txid = self
            .env
            .flows
            .claim(&record, &keys, &preimage)
            .await
            .map_err(|e| SwapError::from_core(e, Some(&record.id)))?;
        self.persist(|r| {
            r.phase = SwapPhase::Claiming;
            r.state.claim_txid = Some(txid.clone());
        })
        .await
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
        let txid = self
            .env
            .flows
            .refund(&record, &keys)
            .await
            .map_err(|e| SwapError::from_core(e, Some(&record.id)))?;
        self.persist(|r| {
            r.phase = SwapPhase::Refunding;
            r.state.refund_txid = Some(txid.clone());
        })
        .await
    }

    /// Unwinds a `Refundable` swap. The driver does this on its own; it is
    /// here for a caller who wants to decide.
    pub async fn refund(&self) -> Result<SwapRecord, SwapError> {
        let record = self.record();
        if record.state.refund_txid.is_some() {
            return Ok(record);
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
    /// an error.
    pub async fn watch<F>(&self, mut on_phase: F) -> Result<SwapRecord, SwapError>
    where
        F: FnMut(SwapPhase) + Send,
    {
        self.drain_transitions();
        loop {
            let record = self.tick().await?;
            for phase in self.drain_transitions() {
                on_phase(phase);
            }
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
}

pub struct KaleidoSwap {
    env: Arc<Env>,
}

impl KaleidoSwap {
    /// The constructor. `maker_url` must be given on a network KaleidoSwap
    /// runs no maker on.
    pub fn new(options: KaleidoSwapOptions) -> Result<Self, SwapError> {
        let network = options.network;
        let api = match &options.maker_url {
            Some(url) => BoltzApiClientV2::new(url.clone(), None),
            None => BoltzApiClientV2::default(network).map_err(|e| {
                SwapError::new(
                    SwapErrorCode::PairUnavailable,
                    format!("KaleidoSwap runs no maker on {network:?}; pass maker_url to name one ({e:?})"),
                )
            })?,
        };
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
            }),
        }
    }

    pub fn network(&self) -> Network {
        self.env.network
    }

    pub async fn quote(&self, request: QuoteRequest) -> Result<QuoteAnswer, SwapError> {
        let (from, to) = (request.from_asset, request.to_asset);
        let refusal = |message: String| {
            QuoteAnswer::Refusal(Refusal {
                from_asset: from,
                to_asset: to,
                reason: QuoteRefusalReason::PairUnavailable,
                limits: None,
                message,
            })
        };
        let on_chain = |a: AssetId| matches!(a, AssetId::BitcoinBtc | AssetId::LiquidBtc);
        if from == AssetId::LightningBtc && on_chain(to) {
            return self.quote_reverse(request).await;
        }
        if on_chain(from) && to == AssetId::LightningBtc {
            return self.quote_submarine(request).await;
        }
        if from.layer() == Layer::Arkade || to.layer() == Layer::Arkade {
            return Ok(refusal(format!(
                "{from} -> {to} rides the Arkade RFQ corridor, which this layer does not route yet; use the corridor module"
            )));
        }
        if from == AssetId::LiquidUsdt || to == AssetId::LiquidUsdt {
            return Ok(refusal(format!(
                "{from} -> {to} needs a caller-funded Liquid spend, which this layer does not route yet; use SwapScript::prepare_liquid_claim"
            )));
        }
        Ok(refusal(format!(
            "{from} -> {to} is not a route the maker serves"
        )))
    }

    fn expires_at(&self) -> u64 {
        unix_now() + self.env.quote_ttl.as_secs()
    }

    async fn quote_reverse(&self, request: QuoteRequest) -> Result<QuoteAnswer, SwapError> {
        let (from, to) = (request.from_asset, request.to_asset);
        let Some(amount) = request.amount else {
            return Err(SwapError::new(
                SwapErrorCode::InvalidAmount,
                "amount is required for a Lightning-funded swap",
            ));
        };
        let pairs = self
            .env
            .maker
            .reverse_pairs()
            .await
            .map_err(|e| SwapError::from_core(e, None))?;
        let Some(card): Option<ReversePair> = pairs.get(from.symbol(), to.symbol()).cloned() else {
            return Ok(QuoteAnswer::Refusal(Refusal {
                from_asset: from,
                to_asset: to,
                reason: QuoteRefusalReason::PairUnavailable,
                limits: None,
                message: format!("the maker does not advertise {from} -> {to}"),
            }));
        };
        let percent = card.fees.percentage;
        let miner = miner_fee_total(&card.fees.miner_fees);
        let from_amount = if request.side == Some(QuoteSide::To) {
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
        if let Some(refusal) = out_of_limits(from, to, from_amount, limits) {
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

    async fn quote_submarine(&self, request: QuoteRequest) -> Result<QuoteAnswer, SwapError> {
        let (from, to) = (request.from_asset, request.to_asset);
        let Some(invoice) = request.invoice.clone() else {
            return Err(SwapError::new(
                SwapErrorCode::InvalidDestination,
                "a swap to Lightning is priced against the invoice: pass it as `invoice`",
            ));
        };
        check_address(&invoice, to, self.env.network, "invoice")?;
        let Some(to_amount) = decode_invoice(&invoice)? else {
            return Err(SwapError::new(
                SwapErrorCode::InvalidAmount,
                "the invoice carries no amount; a submarine swap needs one",
            ));
        };
        if let Some(amount) = request.amount {
            if amount != to_amount {
                return Err(SwapError::new(
                    SwapErrorCode::InvalidAmount,
                    format!("amount {amount} does not match the invoice's {to_amount} sat"),
                ));
            }
        }
        let pairs = self
            .env
            .maker
            .submarine_pairs()
            .await
            .map_err(|e| SwapError::from_core(e, None))?;
        let Some(card): Option<SubmarinePair> = pairs.get(from.symbol(), to.symbol()).cloned()
        else {
            return Ok(QuoteAnswer::Refusal(Refusal {
                from_asset: from,
                to_asset: to,
                reason: QuoteRefusalReason::PairUnavailable,
                limits: None,
                message: format!("the maker does not advertise {from} -> {to}"),
            }));
        };
        let percent = card.fees.percentage;
        let miner = card.fees.miner_fees;
        let maker_fee = percent_fee(to_amount, percent);
        let from_amount = to_amount + maker_fee + miner;
        let limits = Limits {
            min: card.limits.minimal,
            max: card.limits.maximal,
        };
        if let Some(refusal) = out_of_limits(from, to, to_amount, limits) {
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
            expires_at: self.expires_at(),
            binding: QuoteBinding {
                pair_hash: card.hash,
                invoice: Some(invoice),
                zero_conf_limit: card.limits.maximal_zero_conf,
            },
        }))
    }

    /// Creates the swap against `quote`, verifies the lockup against our own
    /// key, persists the record, then returns. Nothing is funded or paid
    /// here — that is the caller's step, via [`Swap::action`].
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
            quote.to_asset,
            self.env.network,
            "destination",
        )?;
        if !reverse {
            let Some(refund) = &options.refund_address else {
                return Err(SwapError::new(
                    SwapErrorCode::RefundAddressRequired,
                    format!(
                        "a swap from {} needs a refund_address on its chain",
                        quote.from_asset
                    ),
                ));
            };
            check_address(refund, quote.from_asset, self.env.network, "refund_address")?;
        }

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
                .get(quote.from_asset.symbol(), quote.to_asset.symbol())
                .filter(|card| card.hash == quote.binding.pair_hash)
                .cloned()
                .ok_or_else(|| {
                    SwapError::new(
                        SwapErrorCode::QuoteExpired,
                        "the pair terms changed since the quote; ask for a new one",
                    )
                })?;
            let request = CreateReverseRequest {
                from: quote.from_asset.symbol().to_string(),
                to: quote.to_asset.symbol().to_string(),
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
                    from: quote.from_asset.symbol().to_string(),
                    to: quote.to_asset.symbol().to_string(),
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
            from_asset: quote.from_asset,
            to_asset: quote.to_asset,
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
                funded_seen: false,
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
        };
        for record in self.env.store.list().await? {
            if record.phase.is_terminal() {
                continue;
            }
            let swap = Swap::new(self.env.clone(), record);
            let after = swap.tick().await?;
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

fn out_of_limits(from: AssetId, to: AssetId, amount: u64, limits: Limits) -> Option<Refusal> {
    let reason = if amount < limits.min {
        QuoteRefusalReason::AmountBelowMin
    } else if amount > limits.max {
        QuoteRefusalReason::AmountAboveMax
    } else {
        return None;
    };
    Some(Refusal {
        from_asset: from,
        to_asset: to,
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
            "limits": {"minimal": 10_000, "maximal": 1_000_000, "maximalZeroConf": 0},
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
            Ok(serde_json::from_value(serde_json::json!({
                "acceptZeroConf": false,
                "address": "tb1qlockup",
                "bip21": "bitcoin:tb1qlockup?amount=0.000502",
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
        async fn swap_status(&self, _id: &str) -> Result<String, Error> {
            let mut statuses = self.statuses.lock().unwrap();
            if statuses.is_empty() {
                return Err(Error::Generic("script exhausted".into()));
            }
            Ok(if statuses.len() > 1 {
                statuses.remove(0)
            } else {
                statuses[0].clone()
            })
        }
    }

    #[derive(Default)]
    struct FakeFlows {
        verify: Mutex<Vec<(String, String)>>,
        claim: Mutex<Vec<(String, String)>>,
        refund: Mutex<Vec<(String, String)>>,
        reject_lockup: bool,
    }

    fn pubkey_hex(keys: &Keypair) -> String {
        keys.public_key().to_string()
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
        async fn claim(
            &self,
            record: &SwapRecord,
            keys: &Keypair,
            _preimage: &Preimage,
        ) -> Result<String, Error> {
            self.claim
                .lock()
                .unwrap()
                .push((record.id.clone(), pubkey_hex(keys)));
            Ok("claim-txid".into())
        }
        async fn refund(&self, record: &SwapRecord, keys: &Keypair) -> Result<String, Error> {
            self.refund
                .lock()
                .unwrap()
                .push((record.id.clone(), pubkey_hex(keys)));
            Ok("refund-txid".into())
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
            from_asset: AssetId::LightningBtc,
            to_asset: AssetId::LiquidBtc,
            side: None,
            amount: Some(amount),
            invoice: None,
        }
    }

    fn submarine_request(invoice: &str) -> QuoteRequest {
        QuoteRequest {
            from_asset: AssetId::BitcoinBtc,
            to_asset: AssetId::LightningBtc,
            side: None,
            amount: None,
            invoice: Some(invoice.into()),
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
    async fn side_to_inverts_the_fee_formula() {
        let h = harness();
        let q = h
            .kaleido
            .quote(QuoteRequest {
                side: Some(QuoteSide::To),
                amount: Some(49_450),
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

        let arkade = h
            .kaleido
            .quote(QuoteRequest {
                from_asset: AssetId::ArkadeBtc,
                ..reverse_request(1)
            })
            .await
            .unwrap()
            .quote()
            .unwrap_err();
        assert_eq!(arkade.reason, QuoteRefusalReason::PairUnavailable);
        assert!(arkade.message.contains("Arkade"));

        let usdt = h
            .kaleido
            .quote(QuoteRequest {
                to_asset: AssetId::LiquidUsdt,
                ..reverse_request(1)
            })
            .await
            .unwrap()
            .quote()
            .unwrap_err();
        assert_eq!(usdt.reason, QuoteRefusalReason::PairUnavailable);
    }

    #[macros::async_test_all]
    async fn submarine_is_priced_against_the_invoice_and_checks_its_network() {
        let h = harness();
        let q = h
            .kaleido
            .quote(submarine_request(INVOICE_SIGNET_50K))
            .await
            .unwrap()
            .quote()
            .unwrap();
        assert_eq!(q.protocol, Protocol::BoltzSubmarine);
        assert_eq!(q.to_amount, 50_000);
        assert_eq!(q.from_amount, 50_000 + 50 + 150);
        assert_eq!(q.binding.invoice.as_deref(), Some(INVOICE_SIGNET_50K));

        let wrong = h
            .kaleido
            .quote(submarine_request(INVOICE_REGTEST_100K))
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);

        let missing = h
            .kaleido
            .quote(QuoteRequest {
                invoice: None,
                ..submarine_request("")
            })
            .await
            .unwrap_err();
        assert_eq!(missing.code, SwapErrorCode::InvalidDestination);

        let mismatch = h
            .kaleido
            .quote(QuoteRequest {
                amount: Some(1),
                ..submarine_request(INVOICE_SIGNET_50K)
            })
            .await
            .unwrap_err();
        assert_eq!(mismatch.code, SwapErrorCode::InvalidAmount);
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
                    destination: Some("lq1mainnet".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);
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
                    destination: Some("tlq1dest".into()),
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
                    destination: Some("tlq1dest".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch);
        assert!(h.store.get("rev-1").await.unwrap().is_none());
    }

    #[macros::async_test_all]
    async fn each_swap_takes_a_fresh_index_and_expired_quotes_are_refused() {
        let h = harness();
        let q = reverse_quote(&h).await;
        let dest = || StartOptions {
            destination: Some("tlq1a".into()),
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
            destination: Some("tlq1dest".into()),
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
            .quote(submarine_request(INVOICE_SIGNET_50K))
            .await
            .unwrap()
            .quote()
            .unwrap();
        *h.maker.submarine_expected.lock().unwrap() = Some(q.from_amount + 1);
        let refund = StartOptions {
            refund_address: Some("tb1qrefund".into()),
            ..Default::default()
        };
        let err = h.kaleido.start(&q, refund).await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::LockupMismatch, "{err}");
        assert!(h.store.list().await.unwrap().is_empty());
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
                    destination: Some("tlq1dest".into()),
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
                    destination: Some("tlq1dest".into()),
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
                    destination: Some("tlq1dest".into()),
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
        let h = harness();
        let q = h
            .kaleido
            .quote(submarine_request(INVOICE_SIGNET_50K))
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
                    refund_address: Some("bc1mainnet".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(wrong.code, SwapErrorCode::WrongNetworkAddress);

        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qrefund".into()),
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
                zero_conf_limit: Some(0),
                invoice: None,
            }
        );
        assert_eq!(
            h.maker.created.lock().unwrap()[0]["invoice"],
            INVOICE_SIGNET_50K
        );

        h.maker.script(&[
            "transaction.mempool",
            "invoice.failedToPay",
            "invoice.failedToPay",
            "transaction.refunded",
        ]);
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
    }

    #[macros::async_test_all]
    async fn submarine_expiry_before_funding_is_failed_not_refundable() {
        let h = harness();
        let q = h
            .kaleido
            .quote(submarine_request(INVOICE_SIGNET_50K))
            .await
            .unwrap()
            .quote()
            .unwrap();
        let swap = h
            .kaleido
            .start(
                &q,
                StartOptions {
                    refund_address: Some("tb1qrefund".into()),
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
                    destination: Some("tlq1dest".into()),
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
        assert!(quiet.resumed.is_empty() && quiet.settled.is_empty());
    }

    #[macros::async_test_all]
    async fn unknown_ids_are_coded() {
        let err = harness().kaleido.swap("nope").await.unwrap_err();
        assert_eq!(err.code, SwapErrorCode::SwapNotFound);
    }

    #[macros::test_all]
    fn asset_ids_round_trip_and_plain_ids_are_checked() {
        for asset in AssetId::ALL {
            assert_eq!(asset.as_str().parse::<AssetId>().unwrap(), asset);
            assert_eq!(serde_json::to_value(asset).unwrap(), asset.as_str());
        }
        assert!("nope".parse::<AssetId>().is_err());
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
                    destination: Some("tlq1dest".into()),
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
