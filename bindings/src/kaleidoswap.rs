//! The high-level surface — `quote`, `start`, `resume` — for Python and React
//! Native. Mirrors `kaleidorg_swap_sdk::kaleidoswap` record for record; the
//! phase machine, the pricing and every check live there, so this file only
//! carries values across the boundary.
//!
//! The store and the signer are *foreign* traits: the application implements
//! them (a database, a secure enclave) and hands them in. Records cross the
//! store as JSON strings, persisted verbatim. `FileStore`, `MemoryStore` and
//! `MnemonicSigner` are Rust implementations of the same traits for callers
//! who want the defaults.

use std::sync::Arc;

use kaleidorg_swap_sdk::kaleidoswap::{
    self as core, AssetId, Fees, Limits, Protocol, Quote, QuoteBinding, QuoteRefusalReason,
    QuoteRequest, QuoteSide, Refusal, StartOptions, SwapAction, SwapErrorCode, SwapPhase,
};
use kaleidorg_swap_sdk::util::secrets::Preimage as CorePreimage;

use crate::boltz::Error;
use kaleidorg_swap_sdk::network::Network;

// ---------------------------------------------------------------------------
// Enums and records, declared as the core declares them.

#[uniffi::remote(Enum)]
pub enum AssetId {
    BitcoinBtc,
    LightningBtc,
    LiquidBtc,
    LiquidUsdt,
    ArkadeBtc,
}

#[uniffi::remote(Enum)]
pub enum QuoteSide {
    From,
    To,
}

#[uniffi::remote(Enum)]
pub enum Protocol {
    BoltzSubmarine,
    BoltzReverse,
    ArkadeRfq,
    PayThrough,
}

#[uniffi::remote(Enum)]
pub enum QuoteRefusalReason {
    PairUnavailable,
    AmountBelowMin,
    AmountAboveMax,
    NoLiquidity,
    MakerPaused,
}

#[uniffi::remote(Enum)]
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

#[uniffi::remote(Enum)]
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

#[uniffi::remote(Record)]
pub struct QuoteRequest {
    pub from_asset: AssetId,
    pub to_asset: AssetId,
    pub side: Option<QuoteSide>,
    pub amount: Option<u64>,
    pub invoice: Option<String>,
}

#[uniffi::remote(Record)]
pub struct Fees {
    pub maker: u64,
    pub miner: u64,
}

#[uniffi::remote(Record)]
pub struct Limits {
    pub min: u64,
    pub max: u64,
}

#[uniffi::remote(Record)]
pub struct QuoteBinding {
    pub pair_hash: String,
    pub invoice: Option<String>,
    pub zero_conf_limit: u64,
}

#[uniffi::remote(Record)]
pub struct Quote {
    pub protocol: Protocol,
    pub from_asset: AssetId,
    pub to_asset: AssetId,
    pub from_amount: u64,
    pub to_amount: u64,
    pub fees: Fees,
    pub limits: Limits,
    pub expires_at: u64,
    pub binding: QuoteBinding,
}

#[uniffi::remote(Record)]
pub struct Refusal {
    pub from_asset: AssetId,
    pub to_asset: AssetId,
    pub reason: QuoteRefusalReason,
    pub limits: Option<Limits>,
    pub message: String,
}

/// Exactly one of the two is set.
#[derive(Debug, Clone, uniffi::Record)]
pub struct QuoteAnswer {
    pub quote: Option<Quote>,
    pub refusal: Option<Refusal>,
}

impl From<core::QuoteAnswer> for QuoteAnswer {
    fn from(answer: core::QuoteAnswer) -> Self {
        match answer {
            core::QuoteAnswer::Quote(q) => Self {
                quote: Some(q),
                refusal: None,
            },
            core::QuoteAnswer::Refusal(r) => Self {
                quote: None,
                refusal: Some(r),
            },
        }
    }
}

#[uniffi::remote(Record)]
pub struct StartOptions {
    pub destination: Option<String>,
    pub refund_address: Option<String>,
}

#[uniffi::remote(Record)]
pub struct SwapAction {
    pub invoice: Option<String>,
    pub funding_address: Option<String>,
    pub funding_amount: Option<u64>,
    pub zero_conf_limit: Option<u64>,
}

/// The persisted record: the fields worth showing, plus the whole record as
/// JSON — which is what a store keeps and what rebuilds the swap later.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SwapRecord {
    pub id: String,
    pub protocol: Protocol,
    pub network: Network,
    pub maker_url: String,
    pub from_asset: AssetId,
    pub to_asset: AssetId,
    pub phase: SwapPhase,
    pub raw_status: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub json: String,
}

impl From<core::SwapRecord> for SwapRecord {
    fn from(r: core::SwapRecord) -> Self {
        let json = serde_json::to_string(&r).unwrap_or_default();
        Self {
            id: r.id,
            protocol: r.protocol,
            network: r.network,
            maker_url: r.maker_url,
            from_asset: r.from_asset,
            to_asset: r.to_asset,
            phase: r.phase,
            raw_status: r.raw_status,
            created_at: r.created_at,
            updated_at: r.updated_at,
            json,
        }
    }
}

impl From<core::SwapError> for Error {
    fn from(e: core::SwapError) -> Self {
        Error::Swap {
            code: e.code,
            message: e.message,
            swap_id: e.swap_id,
        }
    }
}

fn store_err(e: impl std::fmt::Display) -> core::SwapError {
    core::SwapError::new(SwapErrorCode::StoreFailed, e.to_string())
}

fn signer_err(e: impl std::fmt::Display) -> core::SwapError {
    core::SwapError::new(SwapErrorCode::SwapFailed, format!("signer: {e}"))
}

// ---------------------------------------------------------------------------
// Foreign traits: what the application provides.

/// Where the SDK keeps everything a swap needs to finish or unwind after a
/// restart. Records are JSON strings; persist them verbatim. Every write
/// lands before the irreversible step it precedes.
#[uniffi::export(with_foreign)]
pub trait SwapStore: Send + Sync {
    fn get(&self, id: String) -> Result<Option<String>, Error>;
    fn put(&self, id: String, record_json: String) -> Result<(), Error>;
    fn list(&self) -> Result<Vec<String>, Error>;
    /// Hands out the next BIP85 child index, exactly once.
    fn next_index(&self) -> Result<u64, Error>;
}

/// Derives per-swap material from a root the application owns. Hex in, hex
/// out: a 32-byte secret key for `derive_key`, a 32-byte preimage for
/// `derive_preimage`.
#[uniffi::export(with_foreign)]
pub trait SwapSigner: Send + Sync {
    fn derive_key(&self, index: u64) -> Result<String, Error>;
    fn derive_preimage(&self, index: u64) -> Result<String, Error>;
}

/// Called on every phase change while a swap is watched.
#[uniffi::export(with_foreign)]
pub trait PhaseListener: Send + Sync {
    fn on_phase(&self, phase: SwapPhase);
}

struct ForeignStore(Arc<dyn SwapStore>);

#[async_trait::async_trait]
impl core::SwapStore for ForeignStore {
    async fn get(&self, id: &str) -> Result<Option<core::SwapRecord>, core::SwapError> {
        match self.0.get(id.to_string()).map_err(store_err)? {
            Some(json) => serde_json::from_str(&json).map(Some).map_err(store_err),
            None => Ok(None),
        }
    }
    async fn put(&self, record: &core::SwapRecord) -> Result<(), core::SwapError> {
        let json = serde_json::to_string(record).map_err(store_err)?;
        self.0.put(record.id.clone(), json).map_err(store_err)
    }
    async fn list(&self) -> Result<Vec<core::SwapRecord>, core::SwapError> {
        self.0
            .list()
            .map_err(store_err)?
            .iter()
            .map(|json| serde_json::from_str(json).map_err(store_err))
            .collect()
    }
    async fn next_index(&self) -> Result<u64, core::SwapError> {
        self.0.next_index().map_err(store_err)
    }
}

struct ForeignSigner(Arc<dyn SwapSigner>);

#[async_trait::async_trait]
impl core::SwapSigner for ForeignSigner {
    async fn derive_key(&self, index: u64) -> Result<bitcoin::secp256k1::Keypair, core::SwapError> {
        let hex = self.0.derive_key(index).map_err(signer_err)?;
        let secret = bitcoin::secp256k1::SecretKey::from_str(&hex).map_err(signer_err)?;
        Ok(bitcoin::secp256k1::Keypair::from_secret_key(
            &bitcoin::secp256k1::Secp256k1::new(),
            &secret,
        ))
    }
    async fn derive_preimage(&self, index: u64) -> Result<CorePreimage, core::SwapError> {
        let hex = self.0.derive_preimage(index).map_err(signer_err)?;
        let bytes = hex::decode(hex).map_err(signer_err)?;
        CorePreimage::from_vec(bytes).map_err(|e| signer_err(format!("{e:?}")))
    }
}

use std::str::FromStr;

// ---------------------------------------------------------------------------
// Rust implementations of the foreign traits, for callers who want defaults.

/// One JSON file per swap under `dir`, owner-readable only, plus an `index`
/// file for the counter. Single-process.
#[derive(uniffi::Object)]
pub struct FileStore(core::FileStore);

#[uniffi::export]
impl FileStore {
    #[uniffi::constructor]
    pub fn new(dir: String) -> Self {
        Self(core::FileStore::new(dir))
    }
}

/// Nothing survives the process.
#[derive(uniffi::Object)]
pub struct MemoryStore(core::MemoryStore);

#[uniffi::export]
impl MemoryStore {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self(core::MemoryStore::new())
    }
}

fn block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
    // The core store traits are async; the Rust defaults do no I/O that
    // needs a reactor, so a current-thread executor is enough and keeps the
    // foreign trait sync.
    futures_lite_block_on(fut)
}

fn futures_lite_block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop_raw() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            noop_raw()
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    let waker = unsafe { Waker::from_raw(noop_raw()) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

#[uniffi::export]
impl SwapStore for FileStore {
    fn get(&self, id: String) -> Result<Option<String>, Error> {
        let record = block_on(core::SwapStore::get(&self.0, &id))?;
        Ok(record.map(|r| serde_json::to_string(&r).unwrap_or_default()))
    }
    fn put(&self, _id: String, record_json: String) -> Result<(), Error> {
        let record: core::SwapRecord =
            serde_json::from_str(&record_json).map_err(|e| Error::Generic(e.to_string()))?;
        Ok(block_on(core::SwapStore::put(&self.0, &record))?)
    }
    fn list(&self) -> Result<Vec<String>, Error> {
        Ok(block_on(core::SwapStore::list(&self.0))?
            .iter()
            .map(|r| serde_json::to_string(r).unwrap_or_default())
            .collect())
    }
    fn next_index(&self) -> Result<u64, Error> {
        Ok(block_on(core::SwapStore::next_index(&self.0))?)
    }
}

#[uniffi::export]
impl SwapStore for MemoryStore {
    fn get(&self, id: String) -> Result<Option<String>, Error> {
        let record = block_on(core::SwapStore::get(&self.0, &id))?;
        Ok(record.map(|r| serde_json::to_string(&r).unwrap_or_default()))
    }
    fn put(&self, _id: String, record_json: String) -> Result<(), Error> {
        let record: core::SwapRecord =
            serde_json::from_str(&record_json).map_err(|e| Error::Generic(e.to_string()))?;
        Ok(block_on(core::SwapStore::put(&self.0, &record))?)
    }
    fn list(&self) -> Result<Vec<String>, Error> {
        Ok(block_on(core::SwapStore::list(&self.0))?
            .iter()
            .map(|r| serde_json::to_string(r).unwrap_or_default())
            .collect())
    }
    fn next_index(&self) -> Result<u64, Error> {
        Ok(block_on(core::SwapStore::next_index(&self.0))?)
    }
}

/// A [`SwapSigner`] over a swap mnemonic (`SwapMasterKey::from_swap_mnemonic`).
#[derive(uniffi::Object)]
pub struct MnemonicSigner(core::MnemonicSigner);

#[uniffi::export]
impl MnemonicSigner {
    #[uniffi::constructor]
    pub fn new(
        mnemonic: String,
        passphrase: Option<String>,
        network: Network,
    ) -> Result<Self, Error> {
        Ok(Self(core::MnemonicSigner::from_swap_mnemonic(
            &mnemonic,
            passphrase.as_deref(),
            network,
        )?))
    }
}

#[uniffi::export]
impl SwapSigner for MnemonicSigner {
    fn derive_key(&self, index: u64) -> Result<String, Error> {
        let keys = block_on(core::SwapSigner::derive_key(&self.0, index))?;
        Ok(keys.secret_key().display_secret().to_string())
    }
    fn derive_preimage(&self, index: u64) -> Result<String, Error> {
        let preimage = block_on(core::SwapSigner::derive_preimage(&self.0, index))?;
        preimage
            .to_string()
            .ok_or_else(|| Error::Generic("derived preimage has no bytes".into()))
    }
}

// ---------------------------------------------------------------------------
// The instance and a swap in flight.

#[derive(uniffi::Record)]
pub struct KaleidoSwapOptions {
    pub network: Network,
    pub store: Arc<dyn SwapStore>,
    pub signer: Arc<dyn SwapSigner>,
    #[uniffi(default = None)]
    pub maker_url: Option<String>,
    #[uniffi(default = None)]
    pub bitcoin_esplora_url: Option<String>,
    #[uniffi(default = None)]
    pub liquid_esplora_url: Option<String>,
    #[uniffi(default = None)]
    pub poll_interval_ms: Option<u64>,
    #[uniffi(default = None)]
    pub fee_sat_per_vb: Option<f64>,
    #[uniffi(default = None)]
    pub quote_ttl_secs: Option<u64>,
}

#[derive(uniffi::Object)]
pub struct KaleidoSwap {
    inner: core::KaleidoSwap,
}

#[derive(uniffi::Record)]
pub struct ResumeReport {
    pub resumed: Vec<Arc<Swap>>,
    pub settled: Vec<SwapRecord>,
    pub refunded: Vec<SwapRecord>,
    pub failed: Vec<SwapRecord>,
}

#[uniffi::export(async_runtime = "tokio")]
impl KaleidoSwap {
    #[uniffi::constructor]
    pub fn new(options: KaleidoSwapOptions) -> Result<Self, Error> {
        let inner = core::KaleidoSwap::new(core::KaleidoSwapOptions {
            network: options.network,
            store: Arc::new(ForeignStore(options.store)),
            signer: Arc::new(ForeignSigner(options.signer)),
            maker_url: options.maker_url,
            bitcoin_esplora_url: options.bitcoin_esplora_url,
            liquid_esplora_url: options.liquid_esplora_url,
            poll_interval: options
                .poll_interval_ms
                .map(std::time::Duration::from_millis),
            fee_sat_per_vb: options.fee_sat_per_vb,
            quote_ttl: options.quote_ttl_secs.map(std::time::Duration::from_secs),
        })?;
        Ok(Self { inner })
    }

    #[uniffi::method]
    pub fn network(&self) -> Network {
        self.inner.network()
    }

    #[uniffi::method]
    pub async fn quote(&self, request: QuoteRequest) -> Result<QuoteAnswer, Error> {
        Ok(self.inner.quote(request).await?.into())
    }

    #[uniffi::method]
    pub async fn start(&self, quote: Quote, options: StartOptions) -> Result<Arc<Swap>, Error> {
        Ok(Arc::new(Swap(self.inner.start(&quote, options).await?)))
    }

    #[uniffi::method]
    pub async fn swap(&self, id: String) -> Result<Arc<Swap>, Error> {
        Ok(Arc::new(Swap(self.inner.swap(&id).await?)))
    }

    #[uniffi::method]
    pub async fn resume(&self) -> Result<ResumeReport, Error> {
        let report = self.inner.resume().await?;
        Ok(ResumeReport {
            resumed: report
                .resumed
                .into_iter()
                .map(|s| Arc::new(Swap(s)))
                .collect(),
            settled: report.settled.into_iter().map(Into::into).collect(),
            refunded: report.refunded.into_iter().map(Into::into).collect(),
            failed: report.failed.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(uniffi::Object)]
pub struct Swap(core::Swap);

#[uniffi::export(async_runtime = "tokio")]
impl Swap {
    #[uniffi::method]
    pub fn id(&self) -> String {
        self.0.id()
    }

    #[uniffi::method]
    pub fn protocol(&self) -> Protocol {
        self.0.protocol()
    }

    #[uniffi::method]
    pub fn phase(&self) -> SwapPhase {
        self.0.phase()
    }

    #[uniffi::method]
    pub fn action(&self) -> SwapAction {
        self.0.action()
    }

    #[uniffi::method]
    pub fn record(&self) -> SwapRecord {
        self.0.record().into()
    }

    /// One observation of the maker plus whatever action it calls for.
    #[uniffi::method]
    pub async fn tick(&self) -> Result<SwapRecord, Error> {
        Ok(self.0.tick().await?.into())
    }

    #[uniffi::method]
    pub async fn refund(&self) -> Result<SwapRecord, Error> {
        Ok(self.0.refund().await?.into())
    }

    /// Follows the swap to the end; resolves on `Settled`, errors otherwise.
    #[uniffi::method]
    pub async fn settled(&self) -> Result<SwapRecord, Error> {
        Ok(self.0.settled().await?.into())
    }

    /// `settled` with a listener for every phase change.
    #[uniffi::method]
    pub async fn watch(&self, listener: Arc<dyn PhaseListener>) -> Result<SwapRecord, Error> {
        Ok(self.0.watch(|phase| listener.on_phase(phase)).await?.into())
    }
}
