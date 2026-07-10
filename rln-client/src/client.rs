//! Hand-written async HTTP client for the RGB Lightning Node (RLN) API.
//!
//! The style mirrors `kaleidoswap_sdk::boltz::BoltzApiClientV2`: a thin reqwest
//! wrapper exposing typed methods over the generated [`crate::types`]. RLN
//! authenticates with a bearer (Biscuit) token, injected on every request when
//! configured.
//!
//! This covers the full RLN surface — a method per spec path (58 endpoints).
//! Adding another endpoint is a three-line method following the same pattern.

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

    // ---- Node lifecycle: backup / restore / password / shutdown ------------

    /// `POST /backup` — back up the node's data to a file, encrypted with a password.
    pub async fn backup(&self, req: types::BackupRequest) -> Result<(), RlnError> {
        self.post_empty("backup", req).await
    }

    /// `POST /restore` — restore the node from a backup file.
    pub async fn restore(&self, req: types::RestoreRequest) -> Result<(), RlnError> {
        self.post_empty("restore", req).await
    }

    /// `POST /changepassword` — change the node's unlock password.
    pub async fn change_password(&self, req: types::ChangePasswordRequest) -> Result<(), RlnError> {
        self.post_empty("changepassword", req).await
    }

    /// `POST /shutdown` — gracefully shut the node down.
    pub async fn shutdown(&self) -> Result<(), RlnError> {
        self.post_empty("shutdown", Value::Object(Default::default()))
            .await
    }

    // ---- BTC on-chain ------------------------------------------------------

    /// `POST /btcbalance` — on-chain BTC balance (settled/future/spendable).
    pub async fn btc_balance(
        &self,
        req: types::BtcBalanceRequest,
    ) -> Result<types::BtcBalanceResponse, RlnError> {
        self.post("btcbalance", req).await
    }

    /// `POST /sendbtc` — send on-chain BTC to an address.
    pub async fn send_btc(
        &self,
        req: types::SendBtcRequest,
    ) -> Result<types::SendBtcResponse, RlnError> {
        self.post("sendbtc", req).await
    }

    /// `POST /listtransactions` — on-chain BTC transactions.
    pub async fn list_transactions(
        &self,
        req: types::ListTransactionsRequest,
    ) -> Result<types::ListTransactionsResponse, RlnError> {
        self.post("listtransactions", req).await
    }

    /// `POST /listunspents` — unspent outputs (colored + vanilla).
    pub async fn list_unspents(
        &self,
        req: types::ListUnspentsRequest,
    ) -> Result<types::ListUnspentsResponse, RlnError> {
        self.post("listunspents", req).await
    }

    /// `POST /createutxos` — create UTXOs to hold RGB allocations.
    pub async fn create_utxos(&self, req: types::CreateUtxosRequest) -> Result<(), RlnError> {
        self.post_empty("createutxos", req).await
    }

    /// `POST /estimatefee` — estimate the fee rate for a confirmation target.
    pub async fn estimate_fee(
        &self,
        req: types::EstimateFeeRequest,
    ) -> Result<types::EstimateFeeResponse, RlnError> {
        self.post("estimatefee", req).await
    }

    // ---- RGB assets: issuance, inflation, metadata & media -----------------

    /// `POST /issueassetnia` — issue a NIA (non-inflatable fungible) asset.
    pub async fn issue_asset_nia(
        &self,
        req: types::IssueAssetNiaRequest,
    ) -> Result<types::IssueAssetNiaResponse, RlnError> {
        self.post("issueassetnia", req).await
    }

    /// `POST /issueassetcfa` — issue a CFA (collectible fungible) asset.
    pub async fn issue_asset_cfa(
        &self,
        req: types::IssueAssetCfaRequest,
    ) -> Result<types::IssueAssetCfaResponse, RlnError> {
        self.post("issueassetcfa", req).await
    }

    /// `POST /issueassetuda` — issue a UDA (unique digital) asset.
    pub async fn issue_asset_uda(
        &self,
        req: types::IssueAssetUdaRequest,
    ) -> Result<types::IssueAssetUdaResponse, RlnError> {
        self.post("issueassetuda", req).await
    }

    /// `POST /issueassetifa` — issue an IFA (inflatable fungible) asset.
    pub async fn issue_asset_ifa(
        &self,
        req: types::IssueAssetIfaRequest,
    ) -> Result<types::IssueAssetIfaResponse, RlnError> {
        self.post("issueassetifa", req).await
    }

    /// `POST /inflate` — inflate the supply of an IFA asset.
    pub async fn inflate(
        &self,
        req: types::InflateRequest,
    ) -> Result<types::InflateResponse, RlnError> {
        self.post("inflate", req).await
    }

    /// `POST /assetmetadata` — metadata for a single RGB asset.
    pub async fn asset_metadata(
        &self,
        req: types::AssetMetadataRequest,
    ) -> Result<types::AssetMetadataResponse, RlnError> {
        self.post("assetmetadata", req).await
    }

    /// `POST /getassetmedia` — fetch an asset's media (hex-encoded bytes).
    pub async fn get_asset_media(
        &self,
        req: types::GetAssetMediaRequest,
    ) -> Result<types::GetAssetMediaResponse, RlnError> {
        self.post("getassetmedia", req).await
    }

    /// `POST /postassetmedia` — upload asset media as a multipart `file` part,
    /// returning its digest for use when issuing a CFA/UDA asset. `file_name`
    /// defaults to `"media"` when not supplied.
    pub async fn post_asset_media(
        &self,
        file_bytes: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<types::PostAssetMediaResponse, RlnError> {
        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name.unwrap_or_else(|| "media".to_string()));
        let form = reqwest::multipart::Form::new().part("file", part);
        let resp = self
            .prepare(self.http.post(self.url("postassetmedia")).multipart(form))
            .send()
            .await?;
        Self::parse(resp).await
    }

    // ---- RGB transfers -----------------------------------------------------

    /// `POST /listtransfers` — transfers for an RGB asset.
    pub async fn list_transfers(
        &self,
        req: types::ListTransfersRequest,
    ) -> Result<types::ListTransfersResponse, RlnError> {
        self.post("listtransfers", req).await
    }

    /// `POST /refreshtransfers` — refresh pending RGB transfers.
    pub async fn refresh_transfers(&self, req: types::RefreshRequest) -> Result<(), RlnError> {
        self.post_empty("refreshtransfers", req).await
    }

    /// `POST /failtransfers` — fail (abandon) pending RGB transfers.
    pub async fn fail_transfers(
        &self,
        req: types::FailTransfersRequest,
    ) -> Result<types::FailTransfersResponse, RlnError> {
        self.post("failtransfers", req).await
    }

    /// `POST /sync` — sync the RGB wallet against the indexer.
    pub async fn sync(&self, req: types::SyncRequest) -> Result<(), RlnError> {
        self.post_empty("sync", req).await
    }

    // ---- Peers & channels (extended) ---------------------------------------

    /// `GET /listpeers` — connected peers.
    pub async fn list_peers(&self) -> Result<types::ListPeersResponse, RlnError> {
        self.get("listpeers").await
    }

    /// `POST /disconnectpeer` — disconnect from a peer.
    pub async fn disconnect_peer(&self, req: types::DisconnectPeerRequest) -> Result<(), RlnError> {
        self.post_empty("disconnectpeer", req).await
    }

    /// `POST /getchannelid` — resolve a temporary channel id to its final id.
    pub async fn get_channel_id(
        &self,
        req: types::GetChannelIdRequest,
    ) -> Result<types::GetChannelIdResponse, RlnError> {
        self.post("getchannelid", req).await
    }

    // ---- Utility -----------------------------------------------------------

    /// `POST /signmessage` — sign a message with the node key.
    pub async fn sign_message(
        &self,
        req: types::SignMessageRequest,
    ) -> Result<types::SignMessageResponse, RlnError> {
        self.post("signmessage", req).await
    }

    /// `POST /sendonionmessage` — send a BOLT onion message.
    pub async fn send_onion_message(
        &self,
        req: types::SendOnionMessageRequest,
    ) -> Result<(), RlnError> {
        self.post_empty("sendonionmessage", req).await
    }

    /// `POST /checkindexerurl` — validate an RGB indexer URL.
    pub async fn check_indexer_url(
        &self,
        req: types::CheckIndexerUrlRequest,
    ) -> Result<types::CheckIndexerUrlResponse, RlnError> {
        self.post("checkindexerurl", req).await
    }

    /// `POST /checkproxyendpoint` — validate an RGB proxy endpoint.
    pub async fn check_proxy_endpoint(
        &self,
        req: types::CheckProxyEndpointRequest,
    ) -> Result<(), RlnError> {
        self.post_empty("checkproxyendpoint", req).await
    }

    /// `POST /revoketoken` — revoke an issued API token.
    pub async fn revoke_token(&self, req: types::RevokeTokenRequest) -> Result<(), RlnError> {
        self.post_empty("revoketoken", req).await
    }
}
