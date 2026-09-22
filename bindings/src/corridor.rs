//! Python surface for the Arkade Intents corridor — the maker's `/v1` RFQ
//! wire. Mirrors `kaleidorg_swap_sdk::corridor` record for record; see that
//! module for what each field means and which checks a client owes before it
//! commits value.

use kaleidorg_swap_sdk::corridor::{
    self, AmountSide, LightningReceiveRequest, LightningSendRequest, QuoteProfile, RefusalReason,
    RfqQuote, RfqRefusal, RfqState, RfqStatus,
};

use crate::boltz::{Error, SwapClient};

#[uniffi::remote(Enum)]
pub enum AmountSide {
    From,
    To,
}

#[uniffi::remote(Enum)]
pub enum RefusalReason {
    UnsupportedPair,
    UnsupportedPayload,
    AmountOutOfRange,
    ExposureCap,
    InvoiceExpired,
    QuoteConflict,
    PricingUnavailable,
    Unknown,
}

#[uniffi::remote(Enum)]
pub enum RfqState {
    Quoted,
    Refused,
    Expired,
    Funded,
    Filling,
    Filled,
    Settled,
    Refunded,
    Stuck,
}

#[uniffi::remote(Record)]
pub struct LightningSendRequest {
    pub rfq_id: String,
    pub invoice: String,
    pub refund_address: String,
    pub client_refund_pubkey: String,
}

#[uniffi::remote(Record)]
pub struct LightningReceiveRequest {
    pub rfq_id: String,
    pub amount_side: AmountSide,
    pub amount: u64,
    pub payment_hash: String,
    pub payout_address: String,
    pub payout_pubkey: String,
    pub claim_packet: Option<String>,
}

#[uniffi::remote(Record)]
pub struct QuoteProfile {
    pub payment_hash: Option<String>,
    pub lockup_address: Option<String>,
    pub invoice: Option<String>,
    pub receiver_pk_script: Option<String>,
    pub solver_refund_pk_script: Option<String>,
}

#[uniffi::remote(Record)]
pub struct RfqQuote {
    pub v: u8,
    pub rfq_id: String,
    pub pair: String,
    pub from_amount: u64,
    pub to_amount: u64,
    pub solver_pubkey: String,
    pub valid_until: u64,
    pub refund_locktime: Option<u64>,
    pub profile: QuoteProfile,
}

#[uniffi::remote(Record)]
pub struct RfqRefusal {
    pub v: u8,
    pub rfq_id: String,
    pub reason: RefusalReason,
}

#[uniffi::remote(Record)]
pub struct RfqStatus {
    pub v: u8,
    pub kind: String,
    pub rfq_id: String,
    pub state: RfqState,
    pub updated_at: u64,
    pub profile: QuoteProfile,
}

/// What `POST /v1/swap` answers: exactly one of the two is set.
///
/// A record with two optionals rather than an enum with payloads, because a
/// refusal is the maker's answer and not an error — it must reach the caller
/// as a value — and this is the shape that reads naturally on the Python
/// side (`if answer.refusal: ...`).
#[derive(Debug, Clone, uniffi::Record)]
pub struct RfqAnswer {
    pub quote: Option<RfqQuote>,
    pub refusal: Option<RfqRefusal>,
}

impl From<corridor::RfqAnswer> for RfqAnswer {
    fn from(answer: corridor::RfqAnswer) -> Self {
        match answer {
            corridor::RfqAnswer::Quote(quote) => Self {
                quote: Some(*quote),
                refusal: None,
            },
            corridor::RfqAnswer::Refusal(refusal) => Self {
                quote: None,
                refusal: Some(refusal),
            },
        }
    }
}

/// A fresh `rfq_id`: 32 random bytes, hex. Generate once per negotiation and
/// carry it — every status read is keyed by it.
#[uniffi::export]
pub fn new_rfq_id() -> String {
    corridor::new_rfq_id()
}

#[uniffi::export(async_runtime = "tokio")]
impl SwapClient {
    /// The origin the corridor hangs off — this client's `/v2` base with the
    /// suffix removed. Errors for a base that does not end in `/v2`.
    #[uniffi::method]
    pub fn corridor_url(&self) -> Result<String, Error> {
        Ok(self.inner.corridor_root()?)
    }

    /// Quote `arkade:BTC->lightning:BTC`: the trader funds an Arkade lockup
    /// for the maker to pay the invoice from.
    #[uniffi::method]
    pub async fn quote_lightning_send(
        &self,
        request: LightningSendRequest,
    ) -> Result<RfqAnswer, Error> {
        Ok(self.inner.quote_lightning_send(&request).await?.into())
    }

    /// Quote `lightning:BTC->arkade:BTC`: the maker mints a hold invoice and
    /// locks on Arkade once it is paid, for the trader to claim.
    #[uniffi::method]
    pub async fn quote_lightning_receive(
        &self,
        request: LightningReceiveRequest,
    ) -> Result<RfqAnswer, Error> {
        Ok(self.inner.quote_lightning_receive(&request).await?.into())
    }

    /// `GET /v1/rfq/{rfq_id}`. `None` for an id the maker never issued. Poll
    /// until the state is terminal.
    #[uniffi::method]
    pub async fn rfq_status(&self, rfq_id: String) -> Result<Option<RfqStatus>, Error> {
        Ok(self.inner.rfq_status(&rfq_id).await?)
    }
}
