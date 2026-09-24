use bitcoin::hashes::sha256;
use bitcoin::key::PublicKey;
use bitcoin::secp256k1::PublicKey as Secp256k1PublicKey;
use kaleidorg_swap_sdk::boltz::{
    self, BoltzWsConfig, ChainSwapDetails, CreateChainResponse, CreateReverseResponse, Side,
};
use kaleidorg_swap_sdk::boltz::{
    ChannelInfo, FailureReasonIncorrectAmounts, SubSwapStates, SwapStatus, TransactionInfo,
};
use kaleidorg_swap_sdk::error::Error as CoreError;
use kaleidorg_swap_sdk::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};
use kaleidorg_swap_sdk::network::{Chain, Currency, Network};
use kaleidorg_swap_sdk::swaps::boltz::*;
use kaleidorg_swap_sdk::util::secrets::Preimage;
use kaleidorg_swap_sdk::LiquidAssetContext;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use uniffi::Record;

#[derive(Debug, Error, uniffi::Enum)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("A caller-provided Liquid policy-asset input is required to pay fees")]
    LiquidFeeAssetRequired,

    #[error("{0}")]
    Generic(String),
}

impl From<CoreError> for Error {
    fn from(err: CoreError) -> Self {
        match err {
            // `message_with_causes`, not `message`: this surface is a string
            // and cannot walk the source chain, and for a request failure the
            // useful half — "connection refused", "dns error" — is a cause
            // below reqwest's own layer.
            CoreError::HTTP(_) => Error::Http(err.message_with_causes()),
            CoreError::LiquidFeeAssetRequired => Error::LiquidFeeAssetRequired,
            _ => Error::Generic(err.message()),
        }
    }
}

#[derive(Debug, uniffi::Object)]
pub struct SwapClient {
    pub(crate) inner: boltz::BoltzApiClientV2,
}

#[uniffi::remote(Record)]
pub struct BoltzWsConfig {
    pub keep_alive_interval: Duration,
    pub reconnect_delay: Duration,
    pub subscription_timeout: Duration,
    pub protocols: Option<Vec<String>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SwapClient {
    #[uniffi::constructor]
    pub fn new(base_url: &str, timeout: Option<u64>) -> Self {
        Self {
            inner: boltz::BoltzApiClientV2::new(
                base_url.to_string(),
                timeout.map(Duration::from_secs),
            ),
        }
    }

    /// Client pointed at the default **KaleidoSwap maker** for `network`.
    ///
    /// `Signet` is the KaleidoSwap maker (settles on Mutinynet — use
    /// `BitcoinSignet` chain access, not testnet3); `Regtest` is the local
    /// harness. Errors on `Testnet` (we run no testnet3 maker — signet is our
    /// testing network) and on `Mainnet` (no mainnet maker yet), rather than
    /// falling back to a third party. To reach any other maker, use `new` with
    /// an explicit `base_url`.
    #[uniffi::constructor]
    pub fn default(network: Network) -> Result<Self, Error> {
        Ok(Self {
            inner: boltz::BoltzApiClientV2::default(network)?,
        })
    }

    /// Client for the **KaleidoSwap maker** that attributes the swaps it creates
    /// to a partner organization.
    ///
    /// `api_key` is the organization key from the partner panel — a
    /// `kld_test_…` or `kld_live_…` value. It answers "which partner
    /// organization created this swap?" and nothing else: it authorizes no
    /// claim, no refund, no fund movement and no panel access. The per-swap
    /// `swap_auth` credential the maker returns on create stays separate and
    /// unchanged.
    ///
    /// A value that cannot be a key is rejected here rather than reaching the
    /// maker as a `401`, which is the same answer a revoked key gets. The key is
    /// bound to `maker_url` and is never sent anywhere else, and `maker_url`
    /// must be `https` unless it is a loopback address — a bearer credential
    /// over plain HTTP is readable by anything on the path.
    ///
    /// The key is a permanent organization credential: keep it on a server, load
    /// it from configuration rather than committing it, and never ship it inside
    /// a mobile or desktop application binary, where every user holds it.
    ///
    /// No exported method returns the secret, and UniFFI renders no string form
    /// of this object at all — it emits `__str__` only for an object that
    /// exports `Display`, and this one does not, so `str(client)` is the default
    /// `<... object at 0x...>`. See `api_key_id` for the half that is safe to
    /// log.
    #[uniffi::constructor]
    pub fn kaleido_maker(
        maker_url: &str,
        api_key: &str,
        timeout: Option<u64>,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner: KaleidoMakerClient::new(KaleidoMakerClientOptions {
                maker_url: maker_url.to_string(),
                api_key: ApiKey::parse(api_key)?,
                timeout: timeout.map(Duration::from_secs),
            })?
            .into_inner(),
        })
    }

    /// The environment the configured organization key is scoped to — `"test"`
    /// or `"live"` — or `None` for an unauthenticated client.
    ///
    /// Worth asserting at start-up: a `kld_test_…` key against a production
    /// maker is refused by the maker, and this says so before any swap is
    /// attempted.
    #[uniffi::method]
    pub fn api_key_environment(&self) -> Option<String> {
        self.inner
            .api_key()
            .map(|key| key.environment().to_string())
    }

    /// The configured organization key's public identifier — the same one the
    /// partner panel shows. Safe to log and to name in a support request; the
    /// secret half is not reachable from here.
    #[uniffi::method]
    pub fn api_key_id(&self) -> Option<String> {
        self.inner.api_key().map(|key| key.key_id().to_string())
    }

    #[uniffi::method]
    pub async fn create_swap(
        &self,
        swap_request: CreateSubmarineRequest,
    ) -> Result<CreateSubmarineResponse, Error> {
        let from_currency = swap_request
            .from
            .resolve_currency(swap_request.from_currency)?;
        let to_currency = swap_request.to.resolve_currency(swap_request.to_currency)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (Currency::LUsdt, _) | (_, Currency::LUsdt)
        ) {
            self.inner
                .get_submarine_pairs()
                .await?
                .expected_liquid_asset_context(from_currency, to_currency)?
        } else {
            None
        };
        let response = self
            .inner
            .post_swap_req(&boltz::CreateSubmarineRequest {
                from: from_currency.to_string(),
                to: to_currency.to_string(),
                invoice: swap_request.invoice.clone(),
                refund_public_key: swap_request.refund_public_key,
                pair_hash: swap_request.pair_hash.clone(),
                referral_id: swap_request.referral_id.clone(),
                webhook: None,
            })
            .await?;
        response.validate_with_currency_and_asset_context(
            &swap_request.invoice,
            &swap_request.refund_public_key,
            swap_request.from,
            Some(from_currency),
            expected_asset_context,
        )?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn create_reverse_swap(
        &self,
        swap_request: CreateReverseRequest,
    ) -> Result<CreateReverseResponse, Error> {
        let from_currency = swap_request
            .from
            .resolve_currency(swap_request.from_currency)?;
        let to_currency = swap_request.to.resolve_currency(swap_request.to_currency)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (Currency::LUsdt, _) | (_, Currency::LUsdt)
        ) {
            self.inner
                .get_reverse_pairs()
                .await?
                .expected_liquid_asset_context(from_currency, to_currency)?
        } else {
            None
        };
        let response = self
            .inner
            .post_reverse_req(boltz::CreateReverseRequest {
                invoice_amount: Some(swap_request.invoice_amount),
                invoice: None,
                from: from_currency.to_string(),
                to: to_currency.to_string(),
                preimage_hash: Some(
                    swap_request
                        .preimage_hash
                        .parse::<sha256::Hash>()
                        .map_err(|e| Error::Generic(e.to_string()))?,
                ),
                claim_public_key: swap_request.claim_public_key,
                pair_hash: swap_request.pair_hash,
                description: swap_request.description,
                description_hash: swap_request.description_hash,
                address: swap_request.address,
                address_signature: swap_request.address_signature,
                referral_id: swap_request.referral_id,
                webhook: None,
            })
            .await?;
        response.validate_with_currency_and_asset_context(
            &Preimage::from_sha256_str(&swap_request.preimage_hash)?,
            &swap_request.claim_public_key,
            swap_request.to,
            Some(to_currency),
            expected_asset_context,
        )?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn create_chain_swap(
        &self,
        swap_request: CreateChainRequest,
    ) -> Result<CreateChainResponse, Error> {
        let from_currency = swap_request
            .from
            .resolve_currency(swap_request.from_currency)?;
        let to_currency = swap_request.to.resolve_currency(swap_request.to_currency)?;
        let expected_asset_context = if matches!(
            (from_currency, to_currency),
            (Currency::LUsdt, _) | (_, Currency::LUsdt)
        ) {
            self.inner
                .get_chain_pairs()
                .await?
                .expected_liquid_asset_context(from_currency, to_currency)?
        } else {
            None
        };
        let (from_asset_context, to_asset_context): (
            Option<LiquidAssetContext>,
            Option<LiquidAssetContext>,
        ) = match (from_currency, to_currency) {
            (Currency::LUsdt, _) => (expected_asset_context, None),
            (_, Currency::LUsdt) => (None, expected_asset_context),
            _ => (None, None),
        };
        let preimage_hash = swap_request
            .preimage_hash
            .parse::<sha256::Hash>()
            .map_err(|e| Error::Generic(e.to_string()))?;
        let response = self
            .inner
            .post_chain_req(boltz::CreateChainRequest {
                from: from_currency.to_string(),
                to: to_currency.to_string(),
                preimage_hash,
                claim_public_key: Some(swap_request.claim_public_key),
                refund_public_key: Some(swap_request.refund_public_key),
                user_lock_amount: swap_request.user_lock_amount,
                server_lock_amount: swap_request.server_lock_amount,
                pair_hash: swap_request.pair_hash,
                referral_id: swap_request.referral_id,
                webhook: None,
            })
            .await?;
        response.validate_with_currency_and_asset_context(
            &swap_request.claim_public_key,
            &swap_request.refund_public_key,
            swap_request.from,
            swap_request.to,
            &preimage_hash,
            Some(from_currency),
            Some(to_currency),
            from_asset_context,
            to_asset_context,
        )?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn get_submarine_pairs(&self) -> Result<GetSubmarinePairsResponse, Error> {
        let response = self.inner.get_submarine_pairs().await?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn get_reverse_pairs(&self) -> Result<GetReversePairsResponse, Error> {
        let response = self.inner.get_reverse_pairs().await?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn get_chain_pairs(&self) -> Result<GetChainPairsResponse, Error> {
        let response = self.inner.get_chain_pairs().await?;
        Ok(response)
    }

    #[uniffi::method]
    pub async fn get_height(&self) -> Result<HeightResponse, Error> {
        Ok(self.inner.get_height().await?)
    }

    #[uniffi::method]
    pub async fn get_fee_estimation(&self) -> Result<GetFeeEstimationResponse, Error> {
        Ok(self.inner.get_fee_estimation().await?)
    }

    /// The BIP21 magic-routing hint an invoice carries, if any. Paying it
    /// settles on-chain and skips the swap entirely.
    #[uniffi::method]
    pub async fn get_mrh_bip21(&self, invoice: &str) -> Result<MrhResponse, Error> {
        Ok(self.inner.get_mrh_bip21(invoice).await?)
    }

    #[uniffi::method]
    pub async fn get_submarine_tx(&self, id: &str) -> Result<SubmarineSwapTxResp, Error> {
        Ok(self.inner.get_submarine_tx(id).await?)
    }

    #[uniffi::method]
    pub async fn get_submarine_preimage(
        &self,
        id: &str,
    ) -> Result<SubmarineSwapPreimageResp, Error> {
        Ok(self.inner.get_submarine_preimage(id).await?)
    }

    #[uniffi::method]
    pub async fn get_reverse_tx(&self, id: &str) -> Result<ReverseSwapTxResp, Error> {
        Ok(self.inner.get_reverse_tx(id).await?)
    }

    #[uniffi::method]
    pub async fn get_chain_txs(&self, id: &str) -> Result<ChainSwapTxResp, Error> {
        Ok(self.inner.get_chain_txs(id).await?)
    }

    /// The swap's current state, and — against the KaleidoSwap maker — its
    /// event history and failure detail.
    #[uniffi::method]
    pub async fn get_swap(&self, swap_id: &str) -> Result<GetSwapResponse, Error> {
        Ok(self.inner.get_swap(swap_id).await?)
    }

    /// The re-quoted server lockup amount for a chain swap whose user lockup
    /// arrived for a different amount than agreed.
    #[uniffi::method]
    pub async fn get_quote(&self, swap_id: &str) -> Result<GetQuoteResponse, Error> {
        Ok(self.inner.get_quote(swap_id).await?)
    }

    /// Accept a chain-swap re-quote at `amount_sat`.
    ///
    /// `swap_auth` is the per-swap credential the KaleidoSwap maker returned
    /// as `swapAuth` on the create response. Accepting commits the maker's
    /// payout, so the maker authorizes it with that credential rather than
    /// with the swap id — which is not a secret. Omit it only for a maker that
    /// issues none (upstream Boltz); against KaleidoSwap the call is rejected
    /// with `401 invalid_swap_auth` and no other route resolves the re-quote,
    /// so the swap runs out its refund path instead.
    ///
    /// Persist `swap_auth` with the swap when you create it. Nothing re-issues
    /// it — [`Self::swap_restore`] authenticates with an XPUB alone and does
    /// not return it.
    #[uniffi::method]
    pub async fn accept_quote(
        &self,
        swap_id: &str,
        amount_sat: u64,
        swap_auth: Option<String>,
    ) -> Result<(), Error> {
        Ok(self
            .inner
            .accept_quote(swap_id, amount_sat, swap_auth.as_deref())
            .await?)
    }

    /// The maker's Lightning nodes, keyed by implementation (`LND`, `CLN`).
    #[uniffi::method]
    pub async fn get_nodes(&self) -> Result<GetNodesResponse, Error> {
        Ok(self.inner.get_nodes().await?)
    }

    /// Every swap the maker has seen for `xpub` — the recovery entry point
    /// after a reinstall, and what a mobile client calls on launch to find
    /// swaps it still owes a claim or a refund.
    #[uniffi::method]
    pub async fn swap_restore(
        &self,
        xpub: String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<Vec<SwapRestoreResponse>, Error> {
        Ok(self
            .inner
            .post_swap_restore(&xpub, derivation_path, gap_limit)
            .await?)
    }

    /// Highest swap-key derivation index the maker has seen for `xpub`
    /// (`-1` if none), so a restored wallet knows where to resume deriving.
    #[uniffi::method]
    pub async fn swap_restore_index(
        &self,
        xpub: String,
        derivation_path: Option<String>,
        gap_limit: Option<u32>,
    ) -> Result<SwapRestoreIndexResponse, Error> {
        Ok(self
            .inner
            .post_swap_restore_index(&xpub, derivation_path, gap_limit)
            .await?)
    }

    #[uniffi::method]
    pub fn ws(&self) -> SwapWsApi {
        SwapWsApi(Arc::new(self.inner.ws(BoltzWsConfig::default())))
    }
}

#[uniffi::remote(Record)]
pub struct TransactionInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u64>,
}

#[uniffi::remote(Record)]
pub struct FailureReasonIncorrectAmounts {
    pub expected: u64,
    pub actual: u64,
}

#[uniffi::remote(Record)]
pub struct ChannelInfo {
    #[serde(rename = "fundingTransactionId")]
    pub funding_transaction_id: String,
    #[serde(rename = "fundingTransactionVout")]
    pub funding_transaction_vout: u64,
}

#[uniffi::remote(Record)]
pub struct SwapStatus {
    pub id: String,
    pub status: String,
    pub zero_conf_rejected: Option<bool>,
    pub transaction: Option<kaleidorg_swap_sdk::boltz::TransactionInfo>,
    pub failure_reason: Option<String>,
    pub failure_details: Option<kaleidorg_swap_sdk::boltz::FailureReasonIncorrectAmounts>,
    pub channel_info: Option<ChannelInfo>,
}

#[derive(Debug, uniffi::Object)]
pub struct SwapWsUpdates(Mutex<Receiver<SwapStatus>>);

#[uniffi::export(async_runtime = "tokio")]
impl SwapWsUpdates {
    #[uniffi::method]
    pub async fn next(self: Arc<Self>) -> Result<SwapStatus, Error> {
        let mut receiver = self.0.lock().await;
        receiver
            .recv()
            .await
            .map_err(|e| Error::Generic(e.to_string()))
    }
}

#[derive(uniffi::Object)]
pub struct SwapWsApi(Arc<boltz::BoltzWsApi>);

#[uniffi::export(async_runtime = "tokio")]
impl SwapWsApi {
    #[uniffi::constructor]
    pub fn new(ws_url: String) -> Self {
        Self(Arc::new(boltz::BoltzWsApi::new(
            ws_url,
            BoltzWsConfig::default(),
        )))
    }

    #[uniffi::method]
    pub async fn run_ws_loop(&self) {
        self.0.clone().run_ws_loop().await;
    }

    #[uniffi::method]
    pub fn updates(&self) -> SwapWsUpdates {
        SwapWsUpdates(Mutex::new(self.0.updates()))
    }

    /// Whether the socket is currently up. A mobile client that was
    /// backgrounded comes back to a dead socket with no error on it; poll this
    /// on resume and fall back to `get_swap` rather than waiting on updates
    /// that will never arrive.
    #[uniffi::method]
    pub async fn is_connected(&self) -> bool {
        self.0.is_connected().await
    }

    #[uniffi::method]
    pub async fn subscribe_swap(&self, swap_id: &str) -> Result<(), Error> {
        self.0.subscribe_swap(swap_id).await.map_err(|e| e.into())
    }
}

#[uniffi::remote(Enum)]
pub enum Side {
    Lockup,
    Claim,
}

#[uniffi::remote(Record)]
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
    pub asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
}

#[uniffi::remote(Enum)]
pub enum SubSwapStates {
    Created,
    TransactionMempool,
    TransactionConfirmed,
    InvoiceSet,
    InvoicePaid,
    InvoicePending,
    InvoiceFailedToPay,
    TransactionClaimed,
    TransactionClaimPending,
    TransactionLockupFailed,
    SwapExpired,
}

#[derive(Debug, Record)]
pub struct CreateSubmarineRequest {
    pub from: Chain,
    pub to: Chain,
    #[uniffi(default = None)]
    pub from_currency: Option<Currency>,
    #[uniffi(default = None)]
    pub to_currency: Option<Currency>,
    pub invoice: String,
    pub refund_public_key: PublicKey,
    #[uniffi(default = None)]
    pub pair_hash: Option<String>,
    #[uniffi(default = None)]
    pub referral_id: Option<String>,
}

#[derive(Debug, Record)]
pub struct CreateReverseRequest {
    pub from: Chain,
    pub to: Chain,
    #[uniffi(default = None)]
    pub from_currency: Option<Currency>,
    #[uniffi(default = None)]
    pub to_currency: Option<Currency>,
    pub preimage_hash: String,
    pub claim_public_key: PublicKey,
    pub invoice_amount: u64,
    /// Rate card the caller priced against, as submarine and chain accept.
    #[uniffi(default = None)]
    pub pair_hash: Option<String>,
    #[uniffi(default = None)]
    pub description: Option<String>,
    #[uniffi(default = None)]
    pub description_hash: Option<String>,
    #[uniffi(default = None)]
    pub address: Option<String>,
    #[uniffi(default = None)]
    pub address_signature: Option<String>,
    #[uniffi(default = None)]
    pub referral_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct Leaf {
    pub output: String,
    pub version: u8,
}

#[uniffi::remote(Record)]
pub struct SwapTree {
    pub claim_leaf: Leaf,
    pub refund_leaf: Leaf,
}

#[uniffi::remote(Record)]
pub struct CreateSubmarineResponse {
    pub accept_zero_conf: bool,
    pub address: String,
    pub bip21: String,
    pub claim_public_key: PublicKey,
    pub expected_amount: u64,
    pub id: String,
    pub referral_id: Option<String>,
    pub swap_tree: SwapTree,
    pub timeout_block_height: u64,
    pub blinding_key: Option<String>,
    pub asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
    /// Per-swap taker credential the KaleidoSwap maker issues once on
    /// creation. No submarine-swap route needs it today; persist it anyway.
    pub swap_auth: Option<String>,
}

#[uniffi::remote(Record)]
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
    pub asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
    /// Per-swap taker credential the KaleidoSwap maker issues once on
    /// creation. No reverse-swap route needs it today; persist it anyway.
    pub swap_auth: Option<String>,
}

#[derive(Debug, Record)]
pub struct CreateChainRequest {
    pub from: Chain,
    pub to: Chain,
    #[uniffi(default = None)]
    pub from_currency: Option<Currency>,
    #[uniffi(default = None)]
    pub to_currency: Option<Currency>,
    pub preimage_hash: String,
    pub claim_public_key: PublicKey,
    pub refund_public_key: PublicKey,
    #[uniffi(default = None)]
    pub user_lock_amount: Option<u64>,
    #[uniffi(default = None)]
    pub server_lock_amount: Option<u64>,
    #[uniffi(default = None)]
    pub pair_hash: Option<String>,
    #[uniffi(default = None)]
    pub referral_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct CreateChainResponse {
    pub id: String,
    pub claim_details: ChainSwapDetails,
    pub lockup_details: ChainSwapDetails,
    /// Per-swap taker credential the KaleidoSwap maker issues once on
    /// creation, and the only thing that can accept a chain re-quote
    /// afterwards. Secret material: persist it with the swap — nothing,
    /// restore included, ever re-issues it.
    pub swap_auth: Option<String>,
}

/// Various limits of swap parameters
#[uniffi::remote(Record)]
pub struct PairLimits {
    /// Maximum swap amount
    pub maximal: u64,
    /// Minimum swap amount
    pub minimal: u64,
    /// Maximum amount allowed for zero-conf
    pub maximal_zero_conf: u64,
}

#[uniffi::remote(Record)]
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

#[uniffi::remote(Record)]
pub struct ReverseLimits {
    /// Maximum swap amount
    pub maximal: u64,
    /// Minimum swap amount
    pub minimal: u64,
}

#[uniffi::remote(Record)]
pub struct PairMinerFees {
    pub lockup: u64,
    pub claim: u64,
}

#[uniffi::remote(Record)]
pub struct ChainMinerFees {
    pub server: u64,
    pub user: PairMinerFees,
}

#[uniffi::remote(Record)]
pub struct ChainFees {
    pub percentage: f64,
    pub miner_fees: ChainMinerFees,
}

#[uniffi::remote(Record)]
pub struct ReverseFees {
    pub percentage: f64,
    pub miner_fees: PairMinerFees,
}

#[uniffi::remote(Record)]
pub struct SubmarineFees {
    /// The percentage of the "send amount" that is charged by Boltz as "Boltz Fee".
    pub percentage: f64,
    /// The network fees charged for locking up and claiming funds onchain. These values are absolute, denominated in 10 ** -8 of the quote asset.
    pub miner_fees: u64,
}

#[uniffi::remote(Record)]
pub struct ChainPair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: PairLimits,
    /// Total fees required for the swap
    pub fees: ChainFees,
    pub from_asset_id: Option<String>,
    pub to_asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct ReversePair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: ReverseLimits,
    /// Total fees required for the swap
    pub fees: ReverseFees,
    pub from_asset_id: Option<String>,
    pub to_asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct SubmarinePair {
    /// Pair hash, representing an id for an asset-pair swap
    pub hash: String,
    /// The exchange rate of the pair
    pub rate: f64,
    /// The swap limits
    pub limits: SubmarinePairLimits,
    /// Total fees required for the swap
    pub fees: SubmarineFees,
    pub from_asset_id: Option<String>,
    pub to_asset_id: Option<String>,
    pub fee_asset_id: Option<String>,
}

#[uniffi::remote(Record)]
pub struct GetSubmarinePairsResponse {
    pub pairs: HashMap<String, HashMap<String, SubmarinePair>>,
}

#[uniffi::remote(Record)]
pub struct GetReversePairsResponse {
    pub pairs: HashMap<String, HashMap<String, ReversePair>>,
}

#[uniffi::remote(Record)]
pub struct GetChainPairsResponse {
    pub pairs: HashMap<String, HashMap<String, ChainPair>>,
}

// ---------------------------------------------------------------------------
// Chain tip, fees, and per-swap lookups
// ---------------------------------------------------------------------------

#[uniffi::remote(Record)]
pub struct HeightResponse {
    #[serde(rename = "BTC")]
    pub btc: u32,
    #[serde(rename = "L-BTC")]
    pub lbtc: u32,
}

#[uniffi::remote(Record)]
pub struct GetFeeEstimationResponse {
    #[serde(rename = "BTC")]
    pub btc: f64,
    #[serde(rename = "L-BTC")]
    pub lbtc: f64,
}

#[uniffi::remote(Record)]
pub struct MrhResponse {
    pub bip21: String,
    pub signature: String,
}

#[uniffi::remote(Record)]
pub struct SubmarineSwapTxResp {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_block_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_eta: Option<u32>,
}

#[uniffi::remote(Record)]
pub struct SubmarineSwapPreimageResp {
    pub preimage: String,
}

#[uniffi::remote(Record)]
pub struct ReverseSwapTxResp {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    pub timeout_block_height: u32,
}

#[uniffi::remote(Record)]
pub struct ChainSwapTx {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
}

#[uniffi::remote(Record)]
pub struct ChainSwapTxTimeout {
    pub block_height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u32>,
}

#[uniffi::remote(Record)]
pub struct ChainSwapTxLock {
    pub transaction: ChainSwapTx,
    pub timeout: ChainSwapTxTimeout,
}

#[uniffi::remote(Record)]
pub struct ChainSwapTxResp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_lock: Option<ChainSwapTxLock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_lock: Option<ChainSwapTxLock>,
}

#[uniffi::remote(Record)]
pub struct TransactionResponse {
    pub id: String,
    pub hex: String,
}

#[uniffi::remote(Record)]
pub struct TransactionOut {
    pub id: String,
    pub vout: u32,
}

#[uniffi::remote(Record)]
pub struct SwapEvent {
    /// What happened, e.g. `invoice_issued`, `expired`.
    pub kind: String,
    /// Unix seconds.
    pub ts: i64,
}

#[uniffi::remote(Record)]
pub struct GetSwapResponse {
    pub status: String,
    pub zero_conf_rejected: Option<bool>,
    pub transaction: Option<TransactionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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

#[uniffi::remote(Record)]
pub struct GetQuoteResponse {
    /// Server lockup amount, in sat
    pub amount: u64,
}

// `Node` carries a `secp256k1::PublicKey`, which is a different type from the
// `bitcoin::key::PublicKey` the swap records use — the custom type registered
// in `swap.rs` does not cover it, so it gets its own lowering here.
uniffi::custom_type!(Secp256k1PublicKey, String, {
    remote,
    lower: |key| key.to_string(),
    try_lift: |val| match Secp256k1PublicKey::from_str(val.as_str()) {
        Ok(key) => Ok(key),
        Err(e) => Err(Error::Generic(e.to_string()).into()),
    },
});

#[uniffi::remote(Record)]
pub struct Node {
    /// The public key
    pub public_key: Secp256k1PublicKey,
    /// The public URIs
    pub uris: Vec<String>,
}

#[uniffi::remote(Record)]
pub struct GetNodesResponse {
    #[serde(rename = "BTC")]
    pub btc: HashMap<String, Node>,
}

// ---------------------------------------------------------------------------
// Recovery (`swap/restore`)
// ---------------------------------------------------------------------------

// Variant order is the FFI wire order — new variants go at the END. See the
// note on `Network` in `network.rs`.
#[uniffi::remote(Enum)]
pub enum SwapRestoreType {
    Reverse,
    Submarine,
    Chain,
}

#[uniffi::remote(Record)]
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

#[uniffi::remote(Record)]
pub struct RefundDetails {
    pub tree: SwapTree,
    pub amount: Option<u64>,
    pub key_index: u32,
    pub transaction: Option<TransactionOut>,
    pub lockup_address: String,
    pub server_public_key: String,
    pub timeout_block_height: u32,
    pub blinding_key: Option<String>,
}

#[uniffi::remote(Record)]
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

#[uniffi::remote(Record)]
pub struct SwapRestoreIndexResponse {
    pub index: i64,
}
