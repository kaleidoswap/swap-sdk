//! Hand-written async HTTP client for the RGB Lightning Node (RLN) API.
//!
//! The style mirrors `kaleidoswap_sdk::boltz::BoltzApiClientV2`: a thin reqwest
//! wrapper exposing typed methods over the generated [`crate::types`]. RLN
//! authenticates with a bearer (Biscuit) token, injected on every request when
//! configured.
//!
//! This is a *curated* surface — the swap/node/RGB endpoints the SDK actually
//! drives, not a 1:1 mirror of all 58 spec paths. Adding another endpoint is a
//! three-line method following the same pattern.

use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::types;

/// Errors returned by [`RlnClient`].
#[derive(Debug)]
pub enum RlnError {
    /// Transport-level failure (connection, timeout, TLS, …).
    Http(String),
    /// The node returned a non-2xx status; `body` is the raw response text.
    Api { status: u16, body: String },
    /// Response body could not be deserialized into the expected type.
    Json(String),
}

impl std::fmt::Display for RlnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RlnError::Http(e) => write!(f, "HTTP error: {e}"),
            RlnError::Api { status, body } => write!(f, "RLN API error {status}: {body}"),
            RlnError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for RlnError {}

impl From<reqwest::Error> for RlnError {
    fn from(e: reqwest::Error) -> Self {
        RlnError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for RlnError {
    fn from(e: serde_json::Error) -> Self {
        RlnError::Json(e.to_string())
    }
}

/// Async client for a single RGB Lightning Node.
#[derive(Debug, Clone)]
pub struct RlnClient {
    base_url: String,
    token: Option<String>,
    http: reqwest::Client,
    timeout: Option<Duration>,
}

impl RlnClient {
    /// Create a client for `base_url` (e.g. `http://localhost:3001`), with an
    /// optional bearer token and per-request timeout.
    pub fn new(
        base_url: impl Into<String>,
        token: Option<String>,
        timeout: Option<Duration>,
    ) -> Self {
        Self::with_client(base_url, token, reqwest::Client::new(), timeout)
    }

    /// Same as [`RlnClient::new`] but reuses an existing reqwest client.
    pub fn with_client(
        base_url: impl Into<String>,
        token: Option<String>,
        http: reqwest::Client,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
            http,
            timeout,
        }
    }

    /// Set or rotate the bearer token (e.g. after `/unlock` issues one).
    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// Attach bearer auth + timeout to a request builder.
    fn prepare(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let rb = match &self.token {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        };
        match self.timeout {
            Some(timeout) => rb.timeout(timeout),
            None => rb,
        }
    }

    async fn parse<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, RlnError> {
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(RlnError::Api {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str::<T>(&body).map_err(|e| RlnError::Json(e.to_string()))
    }

    /// Check the status of a response with an empty (`EmptyResponse`) body.
    async fn check(resp: reqwest::Response) -> Result<(), RlnError> {
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(RlnError::Api {
                status: status.as_u16(),
                body: resp.text().await?,
            })
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, RlnError> {
        let resp = self.prepare(self.http.get(self.url(path))).send().await?;
        Self::parse(resp).await
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: impl Serialize,
    ) -> Result<T, RlnError> {
        let resp = self
            .prepare(self.http.post(self.url(path)).json(&body))
            .send()
            .await?;
        Self::parse(resp).await
    }

    /// POST a body to an endpoint whose response is `EmptyResponse`.
    async fn post_empty(&self, path: &str, body: impl Serialize) -> Result<(), RlnError> {
        let resp = self
            .prepare(self.http.post(self.url(path)).json(&body))
            .send()
            .await?;
        Self::check(resp).await
    }

    /// POST with no request body (endpoints whose request schema is empty).
    async fn post_bodyless<T: DeserializeOwned>(&self, path: &str) -> Result<T, RlnError> {
        self.post(path, Value::Object(Default::default())).await
    }

    // ---- Node lifecycle & info --------------------------------------------

    /// `POST /init` — initialize a fresh node, returning its mnemonic.
    pub async fn init(&self, req: types::InitRequest) -> Result<types::InitResponse, RlnError> {
        self.post("init", req).await
    }

    /// `POST /unlock` — unlock the node with its password.
    pub async fn unlock(&self, req: types::UnlockRequest) -> Result<(), RlnError> {
        self.post_empty("unlock", req).await
    }

    /// `POST /lock` — lock the node.
    pub async fn lock(&self) -> Result<(), RlnError> {
        self.post_empty("lock", Value::Object(Default::default()))
            .await
    }

    /// `GET /nodeinfo` — node pubkey, network and status.
    pub async fn node_info(&self) -> Result<types::NodeInfoResponse, RlnError> {
        self.get("nodeinfo").await
    }

    /// `GET /networkinfo` — chain/network parameters.
    pub async fn network_info(&self) -> Result<types::NetworkInfoResponse, RlnError> {
        self.get("networkinfo").await
    }

    /// `POST /address` — a fresh on-chain address.
    pub async fn address(&self) -> Result<types::AddressResponse, RlnError> {
        self.post_bodyless("address").await
    }

    // ---- Invoices ----------------------------------------------------------

    /// `POST /lninvoice` — create a BOLT11 invoice (optionally RGB-tagged).
    pub async fn ln_invoice(
        &self,
        req: types::LnInvoiceRequest,
    ) -> Result<types::LnInvoiceResponse, RlnError> {
        self.post("lninvoice", req).await
    }

    /// `POST /decodelninvoice` — decode a BOLT11 invoice.
    pub async fn decode_ln_invoice(
        &self,
        req: types::DecodeLnInvoiceRequest,
    ) -> Result<types::DecodeLnInvoiceResponse, RlnError> {
        self.post("decodelninvoice", req).await
    }

    /// `POST /invoicestatus` — status of an invoice by its BOLT11 string.
    pub async fn invoice_status(
        &self,
        req: types::InvoiceStatusRequest,
    ) -> Result<types::InvoiceStatusResponse, RlnError> {
        self.post("invoicestatus", req).await
    }

    // ---- Payments ----------------------------------------------------------

    /// `POST /sendpayment` — pay a BOLT11 invoice.
    pub async fn send_payment(
        &self,
        req: types::SendPaymentRequest,
    ) -> Result<types::SendPaymentResponse, RlnError> {
        self.post("sendpayment", req).await
    }

    /// `POST /getpayment` — look up a payment by hash.
    pub async fn get_payment(
        &self,
        req: types::GetPaymentRequest,
    ) -> Result<types::GetPaymentResponse, RlnError> {
        self.post("getpayment", req).await
    }

    /// `GET /listpayments` — all known payments.
    pub async fn list_payments(&self) -> Result<types::ListPaymentsResponse, RlnError> {
        self.get("listpayments").await
    }

    /// `POST /keysend` — spontaneous (keysend) payment.
    pub async fn keysend(
        &self,
        req: types::KeysendRequest,
    ) -> Result<types::KeysendResponse, RlnError> {
        self.post("keysend", req).await
    }

    // ---- RGB ---------------------------------------------------------------

    /// `POST /rgbinvoice` — create an RGB invoice.
    pub async fn rgb_invoice(
        &self,
        req: types::RgbInvoiceRequest,
    ) -> Result<types::RgbInvoiceResponse, RlnError> {
        self.post("rgbinvoice", req).await
    }

    /// `POST /decodergbinvoice` — decode an RGB invoice.
    pub async fn decode_rgb_invoice(
        &self,
        req: types::DecodeRgbInvoiceRequest,
    ) -> Result<types::DecodeRgbInvoiceResponse, RlnError> {
        self.post("decodergbinvoice", req).await
    }

    /// `POST /listassets` — RGB assets held by the node.
    pub async fn list_assets(
        &self,
        req: types::ListAssetsRequest,
    ) -> Result<types::ListAssetsResponse, RlnError> {
        self.post("listassets", req).await
    }

    /// `POST /assetbalance` — balance of a single RGB asset.
    pub async fn asset_balance(
        &self,
        req: types::AssetBalanceRequest,
    ) -> Result<types::AssetBalanceResponse, RlnError> {
        self.post("assetbalance", req).await
    }

    /// `POST /sendrgb` — send RGB assets on-chain.
    pub async fn send_rgb(
        &self,
        req: types::SendRgbRequest,
    ) -> Result<types::SendRgbResponse, RlnError> {
        self.post("sendrgb", req).await
    }

    // ---- Channels & peers --------------------------------------------------

    /// `GET /listchannels` — open/pending channels.
    pub async fn list_channels(&self) -> Result<types::ListChannelsResponse, RlnError> {
        self.get("listchannels").await
    }

    /// `POST /openchannel` — open a channel to a peer.
    pub async fn open_channel(
        &self,
        req: types::OpenChannelRequest,
    ) -> Result<types::OpenChannelResponse, RlnError> {
        self.post("openchannel", req).await
    }

    /// `POST /closechannel` — cooperatively or forcibly close a channel.
    pub async fn close_channel(&self, req: types::CloseChannelRequest) -> Result<(), RlnError> {
        self.post_empty("closechannel", req).await
    }

    /// `POST /connectpeer` — connect to a peer.
    pub async fn connect_peer(&self, req: types::ConnectPeerRequest) -> Result<(), RlnError> {
        self.post_empty("connectpeer", req).await
    }

    // ---- Swaps (maker / taker) ---------------------------------------------

    /// `POST /makerinit` — initialize a maker-side swap, returning the swap offer.
    pub async fn maker_init(
        &self,
        req: types::MakerInitRequest,
    ) -> Result<types::MakerInitResponse, RlnError> {
        self.post("makerinit", req).await
    }

    /// `POST /makerexecute` — execute a maker-side swap.
    pub async fn maker_execute(&self, req: types::MakerExecuteRequest) -> Result<(), RlnError> {
        self.post_empty("makerexecute", req).await
    }

    /// `POST /taker` — accept a swap as the taker.
    pub async fn taker(&self, req: types::TakerRequest) -> Result<(), RlnError> {
        self.post_empty("taker", req).await
    }

    /// `POST /getswap` — look up a swap by its parameters.
    pub async fn get_swap(
        &self,
        req: types::GetSwapRequest,
    ) -> Result<types::GetSwapResponse, RlnError> {
        self.post("getswap", req).await
    }

    /// `GET /listswaps` — all known swaps.
    pub async fn list_swaps(&self) -> Result<types::ListSwapsResponse, RlnError> {
        self.get("listswaps").await
    }

    /// `POST /decodeswapstring` — decode a swap string into its components.
    pub async fn decode_swapstring(
        &self,
        req: types::DecodeSwapstringRequest,
    ) -> Result<types::DecodeSwapstringResponse, RlnError> {
        self.post("decodeswapstring", req).await
    }
}
