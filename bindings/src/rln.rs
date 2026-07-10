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
}
