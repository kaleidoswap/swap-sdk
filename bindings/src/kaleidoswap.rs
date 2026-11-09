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
    self as core, Amount, Asset, Fees, Limits, Protocol, Quote, QuoteBinding, QuoteRefusalReason,
    QuoteRequest, Refusal, ReverseTimeoutHeadroom, StartOptions, SwapAction, SwapErrorCode,
    SwapPhase,
};
use kaleidorg_swap_sdk::util::secrets::Preimage as CorePreimage;

use crate::boltz::Error;
use kaleidorg_swap_sdk::network::Network;

// ---------------------------------------------------------------------------
// Enums and records, declared as the core declares them.

#[uniffi::remote(Enum)]
pub enum Asset {
    BtcOnchain,
    BtcLightning,
    BtcLiquid,
    UsdtLiquid,
    BtcArkade,
    BtcSpark,
    RgbOnchain { contract_id: String },
    RgbLightning { contract_id: String },
    SparkToken { token_id: String },
}

#[uniffi::remote(Enum)]
pub enum Amount {
    Pay(u64),
    Receive(u64),
    Invoice(String),
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
    ChainUnreachable,
    InvoiceExpired,
}

#[uniffi::remote(Record)]
pub struct QuoteRequest {
    pub pay: Asset,
    pub receive: Asset,
    pub amount: Amount,
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
    pub from_asset: Asset,
    pub to_asset: Asset,
    pub from_amount: u64,
    pub to_amount: u64,
    pub fees: Fees,
    pub limits: Limits,
    pub expires_at: u64,
    pub binding: QuoteBinding,
}

#[uniffi::remote(Record)]
pub struct Refusal {
    pub from_asset: Asset,
    pub to_asset: Asset,
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

/// Per-chain overrides, in blocks, of the headroom `start` requires of a
/// reverse swap's lockup timeout. A chain left unset keeps its default (12
/// on Bitcoin, 120 on Liquid); a value below one and a half times the claim
/// safety margin (9, 90) is raised to it.
#[uniffi::remote(Record)]
pub struct ReverseTimeoutHeadroom {
    #[uniffi(default = None)]
    pub bitcoin_blocks: Option<u32>,
    #[uniffi(default = None)]
    pub liquid_blocks: Option<u32>,
}

#[uniffi::remote(Record)]
pub struct SwapAction {
    pub invoice: Option<String>,
    pub funding_address: Option<String>,
    pub funding_amount: Option<u64>,
    pub bip21: Option<String>,
    pub zero_conf_limit: Option<u64>,
}

/// The persisted record's fields worth showing. The record itself, which carries
/// the maker's per-swap credential, reaches a store only through
/// `SwapStore::put`, never through this type.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SwapRecord {
    pub id: String,
    pub protocol: Protocol,
    pub network: Network,
    pub maker_url: String,
    pub from_asset: Asset,
    pub to_asset: Asset,
    pub phase: SwapPhase,
    pub raw_status: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<core::SwapRecord> for SwapRecord {
    fn from(r: core::SwapRecord) -> Self {
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

/// A cooperative MuSig2 key-path signing session, hex-encoded: the two
/// 33-byte public keys in aggregation order, the 32-byte Taproot tweak and the
/// 32-byte sighash.
#[derive(Debug, Clone, uniffi::Record)]
pub struct MusigSession {
    pub pubkeys: Vec<String>,
    pub tweak: String,
    pub sighash: String,
}

/// Holds the per-swap keys, one per store index, and signs with them; the SDK
/// never asks for a secret key, so the keys can stay in a hardware wallet, a
/// secure enclave or a remote signer. Hex in, hex out.
///
/// `musig_nonce` generates and keeps a fresh secret nonce and returns the
/// 66-byte public nonce; `musig_partial_sign` signs with the secret nonce
/// behind `our_nonce` and forgets it. Never sign twice with one nonce, and
/// never take a secret nonce from outside: either reveals the key.
#[uniffi::export(with_foreign)]
pub trait SwapSigner: Send + Sync {
    /// The 33-byte compressed public key of swap `index`.
    fn public_key(&self, index: u64) -> Result<String, Error>;
    /// The 32-byte preimage of swap `index`.
    fn derive_preimage(&self, index: u64) -> Result<String, Error>;
    /// A 64-byte BIP340 signature by the untweaked key of swap `index` over
    /// a 32-byte Taproot script-path sighash.
    fn sign_schnorr(&self, index: u64, sighash: String) -> Result<String, Error>;
    /// Starts a MuSig2 signature for `session`: the 66-byte public nonce.
    fn musig_nonce(&self, index: u64, session: MusigSession) -> Result<String, Error>;
    /// Finishes it: the 32-byte partial signature.
    fn musig_partial_sign(
        &self,
        index: u64,
        session: MusigSession,
        our_nonce: String,
        their_nonce: String,
    ) -> Result<String, Error>;
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

impl From<&core::MusigSession> for MusigSession {
    fn from(session: &core::MusigSession) -> Self {
        Self {
            pubkeys: session.pubkeys.iter().map(|k| k.to_string()).collect(),
            tweak: hex::encode(session.tweak),
            sighash: hex::encode(session.sighash),
        }
    }
}

impl TryFrom<MusigSession> for core::MusigSession {
    type Error = Error;
    fn try_from(session: MusigSession) -> Result<Self, Error> {
        let [a, b] = <[String; 2]>::try_from(session.pubkeys)
            .map_err(|_| Error::Generic("a MuSig2 session has two public keys".into()))?;
        Ok(Self {
            pubkeys: [parse_pubkey(&a)?, parse_pubkey(&b)?],
            tweak: hex32(&session.tweak, "tweak")?,
            sighash: hex32(&session.sighash, "sighash")?,
        })
    }
}

fn parse_pubkey(hex: &str) -> Result<PublicKey, Error> {
    PublicKey::from_str(hex).map_err(|e| Error::Generic(format!("public key: {e}")))
}

fn hex32(hex: &str, what: &str) -> Result<[u8; 32], Error> {
    hex::decode(hex)
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| Error::Generic(format!("{what} must be 32 bytes of hex")))
}

fn parse_nonce(hex: &str) -> Result<musig::PublicNonce, Error> {
    musig::PublicNonce::from_str(hex).map_err(|e| Error::Generic(format!("public nonce: {e:?}")))
}

struct ForeignSigner(Arc<dyn SwapSigner>);

#[async_trait::async_trait]
impl core::SwapSigner for ForeignSigner {
    async fn public_key(&self, index: u64) -> Result<PublicKey, core::SwapError> {
        let hex = self.0.public_key(index).map_err(signer_err)?;
        PublicKey::from_str(&hex).map_err(signer_err)
    }
    async fn derive_preimage(&self, index: u64) -> Result<CorePreimage, core::SwapError> {
        let hex = self.0.derive_preimage(index).map_err(signer_err)?;
        let bytes = hex::decode(hex).map_err(signer_err)?;
        CorePreimage::from_vec(bytes).map_err(|e| signer_err(format!("{e:?}")))
    }
    async fn sign_schnorr(
        &self,
        index: u64,
        sighash: [u8; 32],
    ) -> Result<schnorr::Signature, core::SwapError> {
        let hex = self
            .0
            .sign_schnorr(index, hex::encode(sighash))
            .map_err(signer_err)?;
        schnorr::Signature::from_str(&hex).map_err(signer_err)
    }
    async fn musig_nonce(
        &self,
        index: u64,
        session: &core::MusigSession,
    ) -> Result<musig::PublicNonce, core::SwapError> {
        let hex = self
            .0
            .musig_nonce(index, session.into())
            .map_err(signer_err)?;
        parse_nonce(&hex).map_err(signer_err)
    }
    async fn musig_partial_sign(
        &self,
        index: u64,
        session: &core::MusigSession,
        our_nonce: &musig::PublicNonce,
        their_nonce: &musig::PublicNonce,
    ) -> Result<musig::PartialSignature, core::SwapError> {
        let hex = self
            .0
            .musig_partial_sign(
                index,
                session.into(),
                hex::encode(our_nonce.serialize()),
                hex::encode(their_nonce.serialize()),
            )
            .map_err(signer_err)?;
        musig::PartialSignature::from_str(&hex).map_err(|e| signer_err(format!("{e:?}")))
    }
}

use bitcoin::secp256k1::{schnorr, PublicKey};
use core::musig;
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
    fn public_key(&self, index: u64) -> Result<String, Error> {
        Ok(block_on(core::SwapSigner::public_key(&self.0, index))?.to_string())
    }
    fn derive_preimage(&self, index: u64) -> Result<String, Error> {
        let preimage = block_on(core::SwapSigner::derive_preimage(&self.0, index))?;
        preimage
            .to_string()
            .ok_or_else(|| Error::Generic("derived preimage has no bytes".into()))
    }
    fn sign_schnorr(&self, index: u64, sighash: String) -> Result<String, Error> {
        let sighash = hex32(&sighash, "sighash")?;
        Ok(block_on(core::SwapSigner::sign_schnorr(&self.0, index, sighash))?.to_string())
    }
    fn musig_nonce(&self, index: u64, session: MusigSession) -> Result<String, Error> {
        let session = core::MusigSession::try_from(session)?;
        let nonce = block_on(core::SwapSigner::musig_nonce(&self.0, index, &session))?;
        Ok(hex::encode(nonce.serialize()))
    }
    fn musig_partial_sign(
        &self,
        index: u64,
        session: MusigSession,
        our_nonce: String,
        their_nonce: String,
    ) -> Result<String, Error> {
        let session = core::MusigSession::try_from(session)?;
        let partial = block_on(core::SwapSigner::musig_partial_sign(
            &self.0,
            index,
            &session,
            &parse_nonce(&our_nonce)?,
            &parse_nonce(&their_nonce)?,
        ))?;
        Ok(hex::encode(partial.serialize()))
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
    /// Timeout of every request to the maker. Default 30 s.
    #[uniffi(default = None)]
    pub maker_timeout_ms: Option<u64>,
    /// Blocks a reverse swap's lockup must have left before its timeout for
    /// `start` to hand out the invoice. Default 12 on Bitcoin, 120 on Liquid.
    #[uniffi(default = None)]
    pub reverse_timeout_headroom: Option<ReverseTimeoutHeadroom>,
}

#[derive(uniffi::Object)]
pub struct KaleidoSwap {
    inner: core::KaleidoSwap,
}

/// A swap one `resume` pass could not move, with why. It stays in the store;
/// the other swaps are resumed regardless.
#[derive(uniffi::Record)]
pub struct ResumeFailure {
    pub id: String,
    pub code: SwapErrorCode,
    pub message: String,
}

#[derive(uniffi::Record)]
pub struct ResumeReport {
    pub resumed: Vec<Arc<Swap>>,
    pub settled: Vec<SwapRecord>,
    pub refunded: Vec<SwapRecord>,
    pub failed: Vec<SwapRecord>,
    pub errors: Vec<ResumeFailure>,
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
            maker_timeout: options
                .maker_timeout_ms
                .map(std::time::Duration::from_millis),
            reverse_timeout_headroom: options.reverse_timeout_headroom,
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
            errors: report
                .errors
                .into_iter()
                .map(|f| ResumeFailure {
                    id: f.id,
                    code: f.error.code,
                    message: f.error.message,
                })
                .collect(),
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
