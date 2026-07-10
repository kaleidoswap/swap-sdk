//! UniFFI bindings for the RGB Lightning Node (RLN) client.
//!
//! Strategy (the "2+4" approach):
//!  * **#4** — we expose a curated `RlnClient` (the swap/node/RGB operations the
//!    SDK drives), not a blind mirror of all 58 REST paths.
//!  * **#2** — RLN request/response types cross the FFI as JSON via
//!    `uniffi::custom_type!`. Each foreign-language SDK re-hydrates its own
//!    generated model (pydantic / openapi-typescript) through the per-language
//!    `uniffi.toml` `custom_types` config, so callers never see raw JSON and we
//!    write zero hand-mapped mirror types.
//!
//! The single source of truth stays the OpenAPI spec: `rln_client::types`
//! (typify), pydantic, and TS types are all generated from it, so the JSON that
//! flows across this boundary is the spec's own wire format.

use std::sync::Arc;
use std::time::Duration;

use rln_client::types::{
    AddressResponse, AssetBalanceRequest, AssetBalanceResponse, CloseChannelRequest,
    ConnectPeerRequest, DecodeLnInvoiceRequest, DecodeLnInvoiceResponse, DecodeRgbInvoiceRequest,
    DecodeRgbInvoiceResponse, DecodeSwapstringRequest, DecodeSwapstringResponse, GetPaymentRequest,
    GetPaymentResponse, GetSwapRequest, GetSwapResponse, InitRequest, InitResponse,
    InvoiceStatusRequest, InvoiceStatusResponse, KeysendRequest, KeysendResponse,
    ListAssetsRequest, ListAssetsResponse, ListChannelsResponse, ListPaymentsResponse,
    ListSwapsResponse, LnInvoiceRequest, LnInvoiceResponse, MakerExecuteRequest, MakerInitRequest,
    MakerInitResponse, NetworkInfoResponse, NodeInfoResponse, OpenChannelRequest,
    OpenChannelResponse, RgbInvoiceRequest, RgbInvoiceResponse, SendPaymentRequest,
    SendPaymentResponse, SendRgbRequest, SendRgbResponse, TakerRequest, UnlockRequest,
};
// Extended surface — the remaining RLN endpoints (wallet lifecycle, BTC
// on-chain, RGB issuance/media, transfers, peers, utility).
use rln_client::types::{
    AssetMetadataRequest, AssetMetadataResponse, BackupRequest, BtcBalanceRequest,
    BtcBalanceResponse, ChangePasswordRequest, CheckIndexerUrlRequest, CheckIndexerUrlResponse,
    CheckProxyEndpointRequest, CreateUtxosRequest, DisconnectPeerRequest, EstimateFeeRequest,
    EstimateFeeResponse, FailTransfersRequest, FailTransfersResponse, GetAssetMediaRequest,
    GetAssetMediaResponse, GetChannelIdRequest, GetChannelIdResponse, InflateRequest,
    InflateResponse, IssueAssetCfaRequest, IssueAssetCfaResponse, IssueAssetIfaRequest,
    IssueAssetIfaResponse, IssueAssetNiaRequest, IssueAssetNiaResponse, IssueAssetUdaRequest,
    IssueAssetUdaResponse, ListPeersResponse, ListTransactionsRequest, ListTransactionsResponse,
    ListTransfersRequest, ListTransfersResponse, ListUnspentsRequest, ListUnspentsResponse,
    PostAssetMediaResponse, RefreshRequest, RestoreRequest, RevokeTokenRequest, SendBtcRequest,
    SendBtcResponse, SendOnionMessageRequest, SignMessageRequest, SignMessageResponse, SyncRequest,
};

/// Register a batch of `rln_client::types` as UniFFI custom types that cross the
/// FFI boundary as JSON strings. `remote` is required because the concrete types
/// are defined in another crate.
macro_rules! json_ffi_types {
    ($($t:ident),+ $(,)?) => {
        $(
            uniffi::custom_type!($t, String, {
                remote,
                lower: |v| serde_json::to_string(&v).expect(concat!("serialize ", stringify!($t))),
                try_lift: |s| Ok(serde_json::from_str(&s)?),
            });
        )+
    };
}

json_ffi_types!(
    InitRequest,
    InitResponse,
    UnlockRequest,
    NodeInfoResponse,
    NetworkInfoResponse,
    AddressResponse,
    LnInvoiceRequest,
    LnInvoiceResponse,
    DecodeLnInvoiceRequest,
    DecodeLnInvoiceResponse,
    InvoiceStatusRequest,
    InvoiceStatusResponse,
    SendPaymentRequest,
    SendPaymentResponse,
    GetPaymentRequest,
    GetPaymentResponse,
    ListPaymentsResponse,
    KeysendRequest,
    KeysendResponse,
    RgbInvoiceRequest,
    RgbInvoiceResponse,
    DecodeRgbInvoiceRequest,
    DecodeRgbInvoiceResponse,
    ListAssetsRequest,
    ListAssetsResponse,
    AssetBalanceRequest,
    AssetBalanceResponse,
    SendRgbRequest,
    SendRgbResponse,
    ListChannelsResponse,
    OpenChannelRequest,
    OpenChannelResponse,
    CloseChannelRequest,
    ConnectPeerRequest,
    MakerInitRequest,
    MakerInitResponse,
    MakerExecuteRequest,
    TakerRequest,
    GetSwapRequest,
    GetSwapResponse,
    ListSwapsResponse,
    DecodeSwapstringRequest,
    DecodeSwapstringResponse,
    BackupRequest,
    RestoreRequest,
    ChangePasswordRequest,
    BtcBalanceRequest,
    BtcBalanceResponse,
    SendBtcRequest,
    SendBtcResponse,
    ListTransactionsRequest,
    ListTransactionsResponse,
    ListUnspentsRequest,
    ListUnspentsResponse,
    CreateUtxosRequest,
    EstimateFeeRequest,
    EstimateFeeResponse,
    IssueAssetNiaRequest,
    IssueAssetNiaResponse,
    IssueAssetCfaRequest,
    IssueAssetCfaResponse,
    IssueAssetUdaRequest,
    IssueAssetUdaResponse,
    IssueAssetIfaRequest,
    IssueAssetIfaResponse,
    InflateRequest,
    InflateResponse,
    AssetMetadataRequest,
    AssetMetadataResponse,
    GetAssetMediaRequest,
    GetAssetMediaResponse,
    PostAssetMediaResponse,
    ListTransfersRequest,
    ListTransfersResponse,
    RefreshRequest,
    FailTransfersRequest,
    FailTransfersResponse,
    SyncRequest,
    ListPeersResponse,
    DisconnectPeerRequest,
    GetChannelIdRequest,
    GetChannelIdResponse,
    SignMessageRequest,
    SignMessageResponse,
    SendOnionMessageRequest,
    CheckIndexerUrlRequest,
    CheckIndexerUrlResponse,
    CheckProxyEndpointRequest,
    RevokeTokenRequest,
);

/// FFI-facing error type mirroring [`rln_client::RlnError`].
#[derive(Debug, thiserror::Error, uniffi::Enum)]
pub enum RlnError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("RLN API error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("JSON error: {0}")]
    Json(String),
}

impl From<rln_client::RlnError> for RlnError {
    fn from(e: rln_client::RlnError) -> Self {
        match e {
            rln_client::RlnError::Http(s) => RlnError::Http(s),
            rln_client::RlnError::Api { status, body } => RlnError::Api { status, body },
            rln_client::RlnError::Json(s) => RlnError::Json(s),
        }
    }
}

/// Async client for a single RGB Lightning Node, exposed over UniFFI.
#[derive(Debug, uniffi::Object)]
pub struct RlnClient {
    inner: rln_client::RlnClient,
}

#[uniffi::export(async_runtime = "tokio")]
impl RlnClient {
    /// Create a client for `base_url` (e.g. `http://localhost:3001`), with an
    /// optional bearer token and per-request timeout (seconds).
    #[uniffi::constructor]
    pub fn new(base_url: String, token: Option<String>, timeout_secs: Option<u64>) -> Arc<Self> {
        Arc::new(Self {
            inner: rln_client::RlnClient::new(
                base_url,
                token,
                timeout_secs.map(Duration::from_secs),
            ),
        })
    }

    // ---- Node lifecycle & info --------------------------------------------

    pub async fn init(&self, req: InitRequest) -> Result<InitResponse, RlnError> {
        Ok(self.inner.init(req).await?)
    }

    pub async fn unlock(&self, req: UnlockRequest) -> Result<(), RlnError> {
        Ok(self.inner.unlock(req).await?)
    }

    pub async fn lock(&self) -> Result<(), RlnError> {
        Ok(self.inner.lock().await?)
    }

    pub async fn node_info(&self) -> Result<NodeInfoResponse, RlnError> {
        Ok(self.inner.node_info().await?)
    }

    pub async fn network_info(&self) -> Result<NetworkInfoResponse, RlnError> {
        Ok(self.inner.network_info().await?)
    }

    pub async fn address(&self) -> Result<AddressResponse, RlnError> {
        Ok(self.inner.address().await?)
    }

    // ---- Invoices ----------------------------------------------------------

    pub async fn ln_invoice(&self, req: LnInvoiceRequest) -> Result<LnInvoiceResponse, RlnError> {
        Ok(self.inner.ln_invoice(req).await?)
    }

    pub async fn decode_ln_invoice(
        &self,
        req: DecodeLnInvoiceRequest,
    ) -> Result<DecodeLnInvoiceResponse, RlnError> {
        Ok(self.inner.decode_ln_invoice(req).await?)
    }

    pub async fn invoice_status(
        &self,
        req: InvoiceStatusRequest,
    ) -> Result<InvoiceStatusResponse, RlnError> {
        Ok(self.inner.invoice_status(req).await?)
    }

    // ---- Payments ----------------------------------------------------------

    pub async fn send_payment(
        &self,
        req: SendPaymentRequest,
    ) -> Result<SendPaymentResponse, RlnError> {
        Ok(self.inner.send_payment(req).await?)
    }

    pub async fn get_payment(
        &self,
        req: GetPaymentRequest,
    ) -> Result<GetPaymentResponse, RlnError> {
        Ok(self.inner.get_payment(req).await?)
    }

    pub async fn list_payments(&self) -> Result<ListPaymentsResponse, RlnError> {
        Ok(self.inner.list_payments().await?)
    }

    pub async fn keysend(&self, req: KeysendRequest) -> Result<KeysendResponse, RlnError> {
        Ok(self.inner.keysend(req).await?)
    }

    // ---- RGB ---------------------------------------------------------------

    pub async fn rgb_invoice(
        &self,
        req: RgbInvoiceRequest,
    ) -> Result<RgbInvoiceResponse, RlnError> {
        Ok(self.inner.rgb_invoice(req).await?)
    }

    pub async fn decode_rgb_invoice(
        &self,
        req: DecodeRgbInvoiceRequest,
    ) -> Result<DecodeRgbInvoiceResponse, RlnError> {
        Ok(self.inner.decode_rgb_invoice(req).await?)
    }

    pub async fn list_assets(
        &self,
        req: ListAssetsRequest,
    ) -> Result<ListAssetsResponse, RlnError> {
        Ok(self.inner.list_assets(req).await?)
    }

    pub async fn asset_balance(
        &self,
        req: AssetBalanceRequest,
    ) -> Result<AssetBalanceResponse, RlnError> {
        Ok(self.inner.asset_balance(req).await?)
    }

    pub async fn send_rgb(&self, req: SendRgbRequest) -> Result<SendRgbResponse, RlnError> {
        Ok(self.inner.send_rgb(req).await?)
    }

    // ---- Channels & peers --------------------------------------------------

    pub async fn list_channels(&self) -> Result<ListChannelsResponse, RlnError> {
        Ok(self.inner.list_channels().await?)
    }

    pub async fn open_channel(
        &self,
        req: OpenChannelRequest,
    ) -> Result<OpenChannelResponse, RlnError> {
        Ok(self.inner.open_channel(req).await?)
    }

    pub async fn close_channel(&self, req: CloseChannelRequest) -> Result<(), RlnError> {
        Ok(self.inner.close_channel(req).await?)
    }

    pub async fn connect_peer(&self, req: ConnectPeerRequest) -> Result<(), RlnError> {
        Ok(self.inner.connect_peer(req).await?)
    }

    // ---- Swaps (maker / taker) ---------------------------------------------

    pub async fn maker_init(&self, req: MakerInitRequest) -> Result<MakerInitResponse, RlnError> {
        Ok(self.inner.maker_init(req).await?)
    }

    pub async fn maker_execute(&self, req: MakerExecuteRequest) -> Result<(), RlnError> {
        Ok(self.inner.maker_execute(req).await?)
    }

    pub async fn taker(&self, req: TakerRequest) -> Result<(), RlnError> {
        Ok(self.inner.taker(req).await?)
    }

    pub async fn get_swap(&self, req: GetSwapRequest) -> Result<GetSwapResponse, RlnError> {
        Ok(self.inner.get_swap(req).await?)
    }

    pub async fn list_swaps(&self) -> Result<ListSwapsResponse, RlnError> {
        Ok(self.inner.list_swaps().await?)
    }

    pub async fn decode_swapstring(
        &self,
        req: DecodeSwapstringRequest,
    ) -> Result<DecodeSwapstringResponse, RlnError> {
        Ok(self.inner.decode_swapstring(req).await?)
    }

    // ---- Node lifecycle: backup / restore / password / shutdown ------------

    pub async fn backup(&self, req: BackupRequest) -> Result<(), RlnError> {
        Ok(self.inner.backup(req).await?)
    }

    pub async fn restore(&self, req: RestoreRequest) -> Result<(), RlnError> {
        Ok(self.inner.restore(req).await?)
    }

    pub async fn change_password(&self, req: ChangePasswordRequest) -> Result<(), RlnError> {
        Ok(self.inner.change_password(req).await?)
    }

    pub async fn shutdown(&self) -> Result<(), RlnError> {
        Ok(self.inner.shutdown().await?)
    }

    // ---- BTC on-chain ------------------------------------------------------

    pub async fn btc_balance(
        &self,
        req: BtcBalanceRequest,
    ) -> Result<BtcBalanceResponse, RlnError> {
        Ok(self.inner.btc_balance(req).await?)
    }

    pub async fn send_btc(&self, req: SendBtcRequest) -> Result<SendBtcResponse, RlnError> {
        Ok(self.inner.send_btc(req).await?)
    }

    pub async fn list_transactions(
        &self,
        req: ListTransactionsRequest,
    ) -> Result<ListTransactionsResponse, RlnError> {
        Ok(self.inner.list_transactions(req).await?)
    }

    pub async fn list_unspents(
        &self,
        req: ListUnspentsRequest,
    ) -> Result<ListUnspentsResponse, RlnError> {
        Ok(self.inner.list_unspents(req).await?)
    }

    pub async fn create_utxos(&self, req: CreateUtxosRequest) -> Result<(), RlnError> {
        Ok(self.inner.create_utxos(req).await?)
    }

    pub async fn estimate_fee(
        &self,
        req: EstimateFeeRequest,
    ) -> Result<EstimateFeeResponse, RlnError> {
        Ok(self.inner.estimate_fee(req).await?)
    }

    // ---- RGB assets: issuance, inflation, metadata & media -----------------

    pub async fn issue_asset_nia(
        &self,
        req: IssueAssetNiaRequest,
    ) -> Result<IssueAssetNiaResponse, RlnError> {
        Ok(self.inner.issue_asset_nia(req).await?)
    }

    pub async fn issue_asset_cfa(
        &self,
        req: IssueAssetCfaRequest,
    ) -> Result<IssueAssetCfaResponse, RlnError> {
        Ok(self.inner.issue_asset_cfa(req).await?)
    }

    pub async fn issue_asset_uda(
        &self,
        req: IssueAssetUdaRequest,
    ) -> Result<IssueAssetUdaResponse, RlnError> {
        Ok(self.inner.issue_asset_uda(req).await?)
    }

    pub async fn issue_asset_ifa(
        &self,
        req: IssueAssetIfaRequest,
    ) -> Result<IssueAssetIfaResponse, RlnError> {
        Ok(self.inner.issue_asset_ifa(req).await?)
    }

    pub async fn inflate(&self, req: InflateRequest) -> Result<InflateResponse, RlnError> {
        Ok(self.inner.inflate(req).await?)
    }

    pub async fn asset_metadata(
        &self,
        req: AssetMetadataRequest,
    ) -> Result<AssetMetadataResponse, RlnError> {
        Ok(self.inner.asset_metadata(req).await?)
    }

    pub async fn get_asset_media(
        &self,
        req: GetAssetMediaRequest,
    ) -> Result<GetAssetMediaResponse, RlnError> {
        Ok(self.inner.get_asset_media(req).await?)
    }

    /// Upload asset media (`file_bytes`), returning its digest. `file_name`
    /// defaults to `"media"` when omitted.
    pub async fn post_asset_media(
        &self,
        file_bytes: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<PostAssetMediaResponse, RlnError> {
        Ok(self.inner.post_asset_media(file_bytes, file_name).await?)
    }

    // ---- RGB transfers -----------------------------------------------------

    pub async fn list_transfers(
        &self,
        req: ListTransfersRequest,
    ) -> Result<ListTransfersResponse, RlnError> {
        Ok(self.inner.list_transfers(req).await?)
    }

    pub async fn refresh_transfers(&self, req: RefreshRequest) -> Result<(), RlnError> {
        Ok(self.inner.refresh_transfers(req).await?)
    }

    pub async fn fail_transfers(
        &self,
        req: FailTransfersRequest,
    ) -> Result<FailTransfersResponse, RlnError> {
        Ok(self.inner.fail_transfers(req).await?)
    }

    pub async fn sync(&self, req: SyncRequest) -> Result<(), RlnError> {
        Ok(self.inner.sync(req).await?)
    }

    // ---- Peers & channels (extended) ---------------------------------------

    pub async fn list_peers(&self) -> Result<ListPeersResponse, RlnError> {
        Ok(self.inner.list_peers().await?)
    }

    pub async fn disconnect_peer(&self, req: DisconnectPeerRequest) -> Result<(), RlnError> {
        Ok(self.inner.disconnect_peer(req).await?)
    }

    pub async fn get_channel_id(
        &self,
        req: GetChannelIdRequest,
    ) -> Result<GetChannelIdResponse, RlnError> {
        Ok(self.inner.get_channel_id(req).await?)
    }

    // ---- Utility -----------------------------------------------------------

    pub async fn sign_message(
        &self,
        req: SignMessageRequest,
    ) -> Result<SignMessageResponse, RlnError> {
        Ok(self.inner.sign_message(req).await?)
    }

    pub async fn send_onion_message(&self, req: SendOnionMessageRequest) -> Result<(), RlnError> {
        Ok(self.inner.send_onion_message(req).await?)
    }

    pub async fn check_indexer_url(
        &self,
        req: CheckIndexerUrlRequest,
    ) -> Result<CheckIndexerUrlResponse, RlnError> {
        Ok(self.inner.check_indexer_url(req).await?)
    }

    pub async fn check_proxy_endpoint(
        &self,
        req: CheckProxyEndpointRequest,
    ) -> Result<(), RlnError> {
        Ok(self.inner.check_proxy_endpoint(req).await?)
    }

    pub async fn revoke_token(&self, req: RevokeTokenRequest) -> Result<(), RlnError> {
        Ok(self.inner.revoke_token(req).await?)
    }
}
