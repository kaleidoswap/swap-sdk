//! The Arkade Intents corridor — the maker's `/v1` RFQ wire, typed.
//!
//! `arkade:BTC <-> lightning:BTC` is not a Boltz-shaped route. The maker
//! serves it as an RFQ: the client posts an `rfq_request`, the maker answers
//! with a binding `rfq_quote` or an `rfq_refusal`, and the client tracks the
//! swap through `rfq_status`. Funding the quote is the acceptance — there is
//! no accept message — so every field on a quote is final and every check a
//! client wants to make has to happen *before* it commits value.
//!
//! The shapes here mirror `@arkade-os/swap`'s builders and the maker's own
//! `intents/wire.rs` field for field. Ark Labs' reference solver validates its
//! requests with a `.strict()` schema, so a misspelled or extra key is a
//! refusal rather than a warning; the request builders below own the exact
//! spelling so callers never assemble the envelope by hand. Responses are the
//! opposite: unknown fields are tolerated, because a maker adding one must
//! not break every deployed client.
//!
//! ## Where the wire lives
//!
//! The corridor is a **sibling** of `/v2`, not a child of it: `POST /v1/swap`
//! and `GET /v1/rfq/{rfq_id}` hang off the maker's origin. A client built for
//! `https://maker/v2` reaches the corridor at `https://maker/v1/...`, which is
//! what [`corridor_root_from_maker_url`] derives — and it is the one rule the
//! TypeScript `@kaleidorg/swap-sdk/arkade` venue mirrors, so both entries
//! agree on the URL by construction.
//!
//! ## What this module does not do
//!
//! Nothing here touches Arkade itself. A send needs the lockup funded from an
//! Ark wallet and a receive needs the lockup claimed with one; both are the
//! venue's job (`@kaleidorg/swap-sdk/arkade`), which needs `@arkade-os/sdk`.
//! This module is the half every runtime can carry without that dependency:
//! quote, verify, track — enough for a server or a Python host to price and
//! monitor a corridor swap, and for a wallet to decide whether a quote is one
//! it should ever fund.

use std::fmt::{self, Display};
use std::str::FromStr;

use bitcoin::key::rand::{rngs::OsRng, RngCore};
use lightning_invoice::Bolt11Invoice;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::Error;

/// `rfqPair(from, to)` — the from side of the send corridor.
pub const ARKADE_BTC: &str = "arkade:BTC";
/// `rfqPair(from, to)` — the Lightning side.
pub const LIGHTNING_BTC: &str = "lightning:BTC";

/// `arkade:BTC->lightning:BTC` — the trader funds Arkade, the maker pays the
/// invoice.
pub const LIGHTNING_SEND_PAIR: &str = "arkade:BTC->lightning:BTC";
/// `lightning:BTC->arkade:BTC` — the trader pays the maker's hold invoice, the
/// maker locks on Arkade for the trader to claim.
pub const LIGHTNING_RECEIVE_PAIR: &str = "lightning:BTC->arkade:BTC";

/// A client refuses a quote whose refund deadline is nearer than this — the
/// SDK's `MIN_HEADROOM_SECONDS`, 90 minutes. The refund CLTV matures against
/// median-time-past, which lags wall-clock by up to an hour, so a smaller
/// wall-clock margin is no margin at all.
pub const MIN_HEADROOM_SECONDS: u64 = 90 * 60;

/// The claim window a receive-side client demands between the last moment it
/// can pay and the maker's refund deadline — `MIN_CLAIM_WINDOW_SECONDS`.
pub const MIN_CLAIM_WINDOW_SECONDS: u64 = 30 * 60;

/// Which leg of the pair `amount` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AmountSide {
    /// What the trader gives.
    From,
    /// What the trader receives.
    To,
}

/// The two corridor routes this crate knows how to request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorridorRoute {
    LightningSend,
    LightningReceive,
}

impl CorridorRoute {
    /// The wire pair string.
    pub fn pair(self) -> &'static str {
        match self {
            Self::LightningSend => LIGHTNING_SEND_PAIR,
            Self::LightningReceive => LIGHTNING_RECEIVE_PAIR,
        }
    }

    /// Parse a wire pair string. `None` for a pair this crate does not serve —
    /// the onchain and arkade↔arkade corridors exist on the wire but have no
    /// builder here yet.
    pub fn from_pair(pair: &str) -> Option<Self> {
        match pair {
            LIGHTNING_SEND_PAIR => Some(Self::LightningSend),
            LIGHTNING_RECEIVE_PAIR => Some(Self::LightningReceive),
            _ => None,
        }
    }
}

impl Display for CorridorRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.pair())
    }
}

/// A fresh `rfq_id`: 32 random bytes, hex. Unique per negotiation, and the
/// key every later status read is made by, so it is generated once and
/// carried — never re-derived.
pub fn new_rfq_id() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What a client sends to have its BOLT11 paid from an Arkade lockup:
/// `lightningSendRequest`.
///
/// A BOLT11 send is exact-out by construction — the invoice fixes the amount
/// — so the request restates none, and the maker refuses one that does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightningSendRequest {
    /// From [`new_rfq_id`].
    pub rfq_id: String,
    /// The BOLT11 the trader wants paid. Its amount is the swap's `to_amount`.
    pub invoice: String,
    /// The trader's own Ark address — where a refund pays if the maker never
    /// fills. It is pinned into a covenant leaf, so an address and not just a
    /// key.
    pub refund_address: String,
    /// The trader's x-only key (32 bytes, hex) for the covenant's sender-side
    /// leaves. The maker reads it back as the even-Y point.
    pub client_refund_pubkey: String,
}

/// What a client sends to receive on Arkade against a Lightning payment:
/// `lightningReceiveRequest`.
///
/// Nothing fixes the size on this side — the maker mints the invoice — so the
/// trader states an amount and says which leg it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightningReceiveRequest {
    /// From [`new_rfq_id`].
    pub rfq_id: String,
    /// Which leg `amount` names. `To` is what most wallets want — "receive
    /// exactly this on Arkade" — and the maker inverts it through its rate
    /// card, so the price is the free variable and rounds *up* by a sat or
    /// two; assert `to_amount >= amount`, never equality.
    pub amount_side: AmountSide,
    /// Sats, on the `amount_side` leg.
    pub amount: u64,
    /// `sha256(P)` of the trader's OWN preimage, lowercase hex. The maker never
    /// sees `P` until the claim reveals it.
    pub payment_hash: String,
    /// The trader's Arkade payout address — pins the claim covenant leaf.
    pub payout_address: String,
    /// The trader's x-only Arkade key: the covenant's `receiver`, which claims
    /// the lockup.
    pub payout_pubkey: String,
    /// Optional pre-signed claim for the maker's claim daemon, so the trader
    /// can be offline when the lockup lands. Absent where no daemon is
    /// deployed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_packet: Option<String>,
}

/// The `rfq_request` envelope as it goes on the wire.
///
/// Built only by the two builders above, because the maker's peer validates
/// this shape strictly and the spelling is the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqRequest {
    pub v: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub rfq_id: String,
    pub pair: String,
    pub amount_side: AmountSide,
    /// Present on a receive; absent on a send, where the invoice fixes it and
    /// the maker refuses a restated one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
    pub profile: Value,
}

impl RfqRequest {
    /// The route this request asks for, when it is one this crate serves.
    pub fn route(&self) -> Option<CorridorRoute> {
        CorridorRoute::from_pair(&self.pair)
    }
}

impl From<&LightningSendRequest> for RfqRequest {
    fn from(request: &LightningSendRequest) -> Self {
        Self {
            v: 1,
            kind: "rfq_request".to_owned(),
            rfq_id: request.rfq_id.clone(),
            pair: LIGHTNING_SEND_PAIR.to_owned(),
            amount_side: AmountSide::To,
            amount: None,
            profile: json!({
                "invoice": request.invoice,
                "refund_address": request.refund_address,
                "client_refund_pubkey": request.client_refund_pubkey,
            }),
        }
    }
}

impl From<&LightningReceiveRequest> for RfqRequest {
    fn from(request: &LightningReceiveRequest) -> Self {
        let mut profile = json!({
            "payment_hash": request.payment_hash,
            "payout_address": request.payout_address,
            "payout_pubkey": request.payout_pubkey,
        });
        // Omitted rather than `null`: the SDK leaves the key out entirely where
        // no claim daemon is deployed, and a strict peer treats `null` as a
        // value it did not ask for.
        if let Some(packet) = &request.claim_packet {
            profile["claim_packet"] = Value::String(packet.clone());
        }
        Self {
            v: 1,
            kind: "rfq_request".to_owned(),
            rfq_id: request.rfq_id.clone(),
            pair: LIGHTNING_RECEIVE_PAIR.to_owned(),
            amount_side: request.amount_side,
            amount: Some(request.amount),
            profile,
        }
    }
}

/// The route-specific half of a quote.
///
/// Every field is optional on the wire because the two routes fill different
/// ones: a send carries `lockup_address` and `receiver_pk_script`, a receive
/// carries `lockup_address`, `invoice` and `solver_refund_pk_script`. The
/// typed accessors on [`RfqQuote`] say which are binding for which route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QuoteProfile {
    /// `sha256(P)` hex — echoed from the invoice on a send, from the request
    /// on a receive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    /// The covenant the maker expects funded (send) or will fund (receive).
    /// Compare-only: a client derives the same tree from the binding fields
    /// and refuses on mismatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lockup_address: Option<String>,
    /// Receive only: the hold invoice the trader pays to arm the swap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
    /// Send only: the maker's payout destination, pinned into the claim leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver_pk_script: Option<String>,
    /// Receive only: the maker's own refund destination, pinned into the
    /// refund leaf — the one tree parameter nothing else on the wire fixes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver_refund_pk_script: Option<String>,
}

/// An `rfq_quote`: the binding answer. Funding it is the acceptance, so every
/// field here is final.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqQuote {
    pub v: u8,
    pub rfq_id: String,
    pub pair: String,
    /// What the trader gives, sats: the lockup on a send, the invoice on a
    /// receive.
    pub from_amount: u64,
    /// What the trader receives, sats. The maker's fee is the spread between
    /// the two — the protocol carries no fee field.
    pub to_amount: u64,
    /// The maker's key for its side of the covenant, hex.
    pub solver_pubkey: String,
    /// Unix seconds after which the trader must request a fresh quote.
    pub valid_until: u64,
    /// Absolute refund deadline, unix seconds. The trader's recourse on a
    /// send; the *maker's* on a receive, where it bounds the claim window
    /// instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund_locktime: Option<u64>,
    pub profile: QuoteProfile,
}

/// Why the maker will not quote. The wire strings are the SDK's own
/// vocabulary; a reason this crate has not heard of parses as
/// [`RefusalReason::Unknown`] rather than failing the whole answer, since a
/// refusal is still a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    UnsupportedPair,
    UnsupportedPayload,
    AmountOutOfRange,
    ExposureCap,
    InvoiceExpired,
    QuoteConflict,
    PricingUnavailable,
    #[serde(other)]
    Unknown,
}

impl RefusalReason {
    /// Whether the same request could succeed if simply retried: a
    /// [`QuoteConflict`](Self::QuoteConflict) is a raced rate card or a
    /// duplicate payment hash, and a fresh `rfq_id` with a fresh invoice
    /// clears it. Everything else needs a different request or a different
    /// moment.
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::QuoteConflict)
    }
}

/// An `rfq_refusal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqRefusal {
    pub v: u8,
    pub rfq_id: String,
    pub reason: RefusalReason,
}

/// What `POST /v1/swap` answers. A refusal is a `200` carrying `rfq_refusal`,
/// not an HTTP error — the maker's decision, priced and deliberate — so it is
/// a variant here and not an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RfqAnswer {
    /// Boxed only for size: a quote is several strings wide and a refusal is
    /// one word, and the two share an enum.
    #[serde(rename = "rfq_quote")]
    Quote(Box<RfqQuote>),
    #[serde(rename = "rfq_refusal")]
    Refusal(RfqRefusal),
}

impl RfqAnswer {
    /// The quote, or an [`Error::Protocol`] naming the refusal — for callers
    /// that have no separate refusal path.
    pub fn into_quote(self) -> Result<RfqQuote, Error> {
        match self {
            Self::Quote(quote) => Ok(*quote),
            Self::Refusal(refusal) => Err(Error::Protocol(format!(
                "the maker refused rfq {}: {}",
                refusal.rfq_id,
                serde_json::to_value(refusal.reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_else(|| format!("{:?}", refusal.reason)),
            ))),
        }
    }

    pub fn quote(&self) -> Option<&RfqQuote> {
        match self {
            Self::Quote(quote) => Some(quote),
            Self::Refusal(_) => None,
        }
    }

    pub fn refusal(&self) -> Option<&RfqRefusal> {
        match self {
            Self::Quote(_) => None,
            Self::Refusal(refusal) => Some(refusal),
        }
    }
}

/// One of the wire states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RfqState {
    Quoted,
    Refused,
    Expired,
    Funded,
    Filling,
    Filled,
    Settled,
    Refunded,
    /// Deliberately reachable: the maker distinguishes "still working" from
    /// "this needs a human", and a client should stop polling and say so
    /// rather than spin.
    Stuck,
}

impl RfqState {
    /// The states after which no further update will come — the SDK's
    /// `RFQ_TERMINAL_STATES`. Poll until one of these.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Settled | Self::Refused | Self::Expired | Self::Refunded | Self::Stuck
        )
    }

    /// The one terminal state that is a success.
    pub fn is_settled(self) -> bool {
        matches!(self, Self::Settled)
    }
}

/// `GET /v1/rfq/{rfq_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqStatus {
    pub v: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub rfq_id: String,
    pub state: RfqState,
    /// Unix seconds.
    pub updated_at: u64,
    pub profile: QuoteProfile,
}

/// The facts a receive-side client reads off the maker's hold invoice, once
/// [`RfqQuote::verify_receive_invoice`] has accepted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveInvoiceFacts {
    /// The invoice's own amount, sats. Equal to the quote's `from_amount` by
    /// construction — that is what was verified.
    pub amount_sats: u64,
    /// Absolute invoice expiry, unix seconds.
    pub invoice_expires_at: u64,
    /// The last moment the invoice can be paid: `min(invoice expiry,
    /// valid_until)`. Measure the claim window from here, not from now — a
    /// payment at the deadline is the case that has to be safe.
    pub pay_deadline: u64,
}

impl RfqQuote {
    /// The maker's fee, sats: the spread between what the trader gives and
    /// what it receives.
    pub fn fee_sats(&self) -> u64 {
        self.from_amount.saturating_sub(self.to_amount)
    }

    /// The route, when it is one this crate serves.
    pub fn route(&self) -> Option<CorridorRoute> {
        CorridorRoute::from_pair(&self.pair)
    }

    /// The gate a **send**-side client runs immediately before funding the
    /// lockup — `assertFundable`. Never at quote time: the quote can sit on a
    /// confirmation screen, and it is the moment of funding that has to be
    /// safe.
    ///
    /// Refuses a quote that has expired, one with no refund deadline, and one
    /// leaving under [`MIN_HEADROOM_SECONDS`] before that deadline: past it
    /// the maker could let the lockup sit until the refund path opens and
    /// the trader would have paid the on-chain cost of finding out.
    pub fn assert_fundable(&self, now: u64) -> Result<(), Error> {
        if now >= self.valid_until {
            return Err(Error::Protocol(format!(
                "quote {} expired at {} (now {now}) — request a fresh one",
                self.rfq_id, self.valid_until
            )));
        }
        let refund_locktime = self.refund_locktime.ok_or_else(|| {
            Error::Protocol(format!(
                "quote {} carries no refund_locktime — a Lightning corridor quote \
                 always names the trader's refund deadline",
                self.rfq_id
            ))
        })?;
        if refund_locktime.saturating_sub(now) < MIN_HEADROOM_SECONDS {
            return Err(Error::Protocol(format!(
                "quote {} leaves {}s before its refund deadline; refusing under {}s \
                 because the deadline matures against median-time-past, which can \
                 lag the clock by an hour",
                self.rfq_id,
                refund_locktime.saturating_sub(now),
                MIN_HEADROOM_SECONDS
            )));
        }
        Ok(())
    }

    /// The **receive**-side check on the maker's hold invoice —
    /// `verifyReceiveInvoice`. The invoice is the maker's, so before it is
    /// handed to a payer it is checked against the trader's own payment hash
    /// and the quote's `from_amount`: an invoice for another hash pays the
    /// maker for a swap the trader cannot claim, and one for another amount
    /// changes the price after the quote.
    ///
    /// `expected_payment_hash` is `sha256(P)` hex, any case.
    pub fn verify_receive_invoice(
        &self,
        expected_payment_hash: &str,
    ) -> Result<ReceiveInvoiceFacts, Error> {
        let raw = self.profile.invoice.as_deref().ok_or_else(|| {
            Error::Protocol(format!(
                "quote {} carries no invoice — a receive quote must name what the \
                 trader pays",
                self.rfq_id
            ))
        })?;
        let invoice = Bolt11Invoice::from_str(raw.trim())
            .map_err(|e| Error::Protocol(format!("the maker's invoice does not decode: {e}")))?;

        let paid_hash = invoice.payment_hash().to_string();
        if !paid_hash.eq_ignore_ascii_case(expected_payment_hash) {
            return Err(Error::Protocol(format!(
                "the maker's invoice pays {paid_hash}, not this swap's \
                 {expected_payment_hash} — paying it would fund a lockup this trader \
                 cannot claim"
            )));
        }

        let amount_sats = invoice
            .amount_milli_satoshis()
            .map(|msat| msat / 1_000)
            .filter(|sats| *sats > 0)
            .ok_or_else(|| Error::Protocol("the maker's invoice names no amount".to_owned()))?;
        if amount_sats != self.from_amount {
            return Err(Error::Protocol(format!(
                "the maker's invoice asks for {amount_sats} sats, not the quoted \
                 from_amount {}",
                self.from_amount
            )));
        }

        let invoice_expires_at = invoice
            .expires_at()
            .map(|d| d.as_secs())
            .ok_or_else(|| Error::Protocol("the maker's invoice has no expiry".to_owned()))?;

        Ok(ReceiveInvoiceFacts {
            amount_sats,
            invoice_expires_at,
            pay_deadline: invoice_expires_at.min(self.valid_until),
        })
    }

    /// The **receive**-side gate — `assertReceivable`, run before the invoice
    /// is handed to a payer. Separate from [`Self::assert_fundable`] because
    /// the semantics invert: `refund_locktime` is the *maker's* here, so
    /// median-time-past lag extends the trader's claim window rather than
    /// shrinking it, and what can actually run out is the hold invoice's own
    /// window. So the claim window is measured from `pay_deadline` — the
    /// last moment a payer can arm the swap — and not from now.
    ///
    /// `max_pay_sats`, when given, is an absolute ceiling on `from_amount`:
    /// with `amount_side: To` the price is the free variable, and this is
    /// where a caller says how far it may move.
    pub fn assert_receivable(
        &self,
        facts: &ReceiveInvoiceFacts,
        now: u64,
        max_pay_sats: Option<u64>,
    ) -> Result<(), Error> {
        if now >= facts.pay_deadline {
            return Err(Error::Protocol(format!(
                "quote {} can no longer be paid (deadline {}, now {now}) — request a \
                 fresh one",
                self.rfq_id, facts.pay_deadline
            )));
        }
        let refund_locktime = self.refund_locktime.ok_or_else(|| {
            Error::Protocol(format!(
                "quote {} carries no refund_locktime — without the maker's refund \
                 deadline the claim window cannot be checked",
                self.rfq_id
            ))
        })?;
        let window = refund_locktime.saturating_sub(facts.pay_deadline);
        if window < MIN_CLAIM_WINDOW_SECONDS {
            return Err(Error::Protocol(format!(
                "a payment at the deadline would leave {window}s to claim before the \
                 maker's refund opens; refusing under {MIN_CLAIM_WINDOW_SECONDS}s"
            )));
        }
        if let Some(ceiling) = max_pay_sats {
            if self.from_amount > ceiling {
                return Err(Error::Protocol(format!(
                    "quote {} asks {} sats, above the {ceiling}-sat ceiling",
                    self.rfq_id, self.from_amount
                )));
            }
        }
        Ok(())
    }
}

/// The origin the corridor hangs off, from the `/v2` base URL every client in
/// this crate is built with.
///
/// `https://maker.example/v2` → `https://maker.example`. The `/v2` suffix is
/// required rather than stripped when present: a URL without it is not a
/// maker base this crate recognises, and guessing an origin would send the
/// request — and the organization API key riding on it — somewhere the
/// caller did not name. A trailing slash is tolerated; a query string or
/// fragment is not, since neither belongs on a base URL.
pub fn corridor_root_from_maker_url(maker_url: &str) -> Result<String, Error> {
    let url = reqwest::Url::parse(maker_url)?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Protocol(format!(
            "maker URL {maker_url} carries a query or fragment, which a base URL \
             cannot"
        )));
    }
    let path = url.path().trim_end_matches('/');
    let Some(root_path) = path.strip_suffix("/v2") else {
        return Err(Error::Protocol(format!(
            "maker URL {maker_url} does not end in /v2 — the Intents corridor is a \
             sibling of the /v2 routes, so its origin can only be derived from a \
             /v2 base"
        )));
    };
    let mut root = url.clone();
    root.set_path(root_path);
    root.set_query(None);
    root.set_fragment(None);
    Ok(root.as_str().trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::{sha256, Hash};
    use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
    use lightning_invoice::{Currency as LnCurrency, InvoiceBuilder, PaymentSecret};
    use std::time::Duration;

    const XONLY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    /// Byte-for-byte what `lightningSendRequest` builds and what the maker's
    /// own wire test pins — so a rename on either side fails here rather than
    /// against a `.strict()` peer in production.
    #[test]
    fn a_send_request_is_the_exact_shape_the_maker_pins() {
        let request = LightningSendRequest {
            rfq_id: "01J0000000000000000000000".into(),
            invoice: "lnbc1...".into(),
            refund_address: "ark1qexample".into(),
            client_refund_pubkey: XONLY.into(),
        };
        let wire = serde_json::to_value(RfqRequest::from(&request)).unwrap();
        assert_eq!(
            wire,
            json!({
                "v": 1,
                "type": "rfq_request",
                "rfq_id": "01J0000000000000000000000",
                "pair": "arkade:BTC->lightning:BTC",
                "amount_side": "to",
                "profile": {
                    "invoice": "lnbc1...",
                    "refund_address": "ark1qexample",
                    "client_refund_pubkey": XONLY
                }
            })
        );
        // A send never restates an amount: the invoice fixes it, and the
        // maker refuses one that does.
        assert!(wire.get("amount").is_none());
    }

    /// Byte-for-byte what `lightningReceiveRequest` builds.
    #[test]
    fn a_receive_request_is_the_exact_shape_the_maker_pins() {
        let request = LightningReceiveRequest {
            rfq_id: "01J0000000000000000000000".into(),
            amount_side: AmountSide::To,
            amount: 60_000,
            payment_hash: "f934a71bcd3a01806376c6d4c50cc0029f12644b26b201dd77d38071001ca880".into(),
            payout_address: "tark1qexample".into(),
            payout_pubkey: XONLY.into(),
            claim_packet: None,
        };
        let wire = serde_json::to_value(RfqRequest::from(&request)).unwrap();
        assert_eq!(
            wire,
            json!({
                "v": 1,
                "type": "rfq_request",
                "rfq_id": "01J0000000000000000000000",
                "pair": "lightning:BTC->arkade:BTC",
                "amount_side": "to",
                "amount": 60000,
                "profile": {
                    "payment_hash": "f934a71bcd3a01806376c6d4c50cc0029f12644b26b201dd77d38071001ca880",
                    "payout_address": "tark1qexample",
                    "payout_pubkey": XONLY
                }
            })
        );
        // The key is absent, not null — a strict peer refuses `null`.
        assert!(wire["profile"].get("claim_packet").is_none());
    }

    #[test]
    fn a_claim_packet_rides_when_given() {
        let request = LightningReceiveRequest {
            rfq_id: "id".into(),
            amount_side: AmountSide::From,
            amount: 1,
            payment_hash: "00".into(),
            payout_address: "tark1q".into(),
            payout_pubkey: XONLY.into(),
            claim_packet: Some("deadbeef".into()),
        };
        let wire = serde_json::to_value(RfqRequest::from(&request)).unwrap();
        assert_eq!(wire["profile"]["claim_packet"], "deadbeef");
        assert_eq!(wire["amount_side"], "from");
    }

    /// A quote as the maker emits it for a receive, every binding field present.
    fn receive_quote_json() -> Value {
        json!({
            "v": 1,
            "type": "rfq_quote",
            "rfq_id": "abc",
            "pair": "lightning:BTC->arkade:BTC",
            "from_amount": 12121,
            "to_amount": 12000,
            "solver_pubkey": format!("02{XONLY}"),
            "valid_until": 1_800_000_120,
            "refund_locktime": 1_800_009_000,
            "profile": {
                "payment_hash": "f934a71b",
                "lockup_address": "tark1qlockup",
                "invoice": "lnbcrt121210n1...",
                "solver_refund_pk_script": "5120aa"
            }
        })
    }

    #[test]
    fn a_quote_parses_and_tolerates_fields_it_does_not_know() {
        let mut body = receive_quote_json();
        body["surprise"] = json!(true);
        body["profile"]["another"] = json!("x");
        let answer: RfqAnswer = serde_json::from_value(body).unwrap();
        let quote = answer.quote().expect("a quote");
        assert_eq!(quote.from_amount, 12121);
        assert_eq!(quote.fee_sats(), 121);
        assert_eq!(quote.route(), Some(CorridorRoute::LightningReceive));
        assert_eq!(quote.profile.invoice.as_deref(), Some("lnbcrt121210n1..."));
        assert_eq!(quote.profile.receiver_pk_script, None);
    }

    /// A refusal is a `200` carrying `rfq_refusal` — the type is what says
    /// the corridor declined, so it must parse as an answer and not an error.
    #[test]
    fn a_refusal_parses_as_an_answer() {
        let answer: RfqAnswer = serde_json::from_value(json!({
            "v": 1,
            "type": "rfq_refusal",
            "rfq_id": "abc",
            "reason": "pricing_unavailable"
        }))
        .unwrap();
        let refusal = answer.refusal().expect("a refusal");
        assert_eq!(refusal.reason, RefusalReason::PricingUnavailable);
        assert!(!refusal.reason.is_retryable());
        assert!(RefusalReason::QuoteConflict.is_retryable());
        let err = answer.into_quote().unwrap_err();
        assert!(err.message().contains("pricing_unavailable"), "{err:?}");
    }

    /// The maker may grow its vocabulary; a reason this crate has not heard
    /// of is still a refusal, and must not fail the parse.
    #[test]
    fn an_unknown_refusal_reason_still_parses() {
        let answer: RfqAnswer = serde_json::from_value(json!({
            "v": 1, "type": "rfq_refusal", "rfq_id": "abc", "reason": "solver_on_fire"
        }))
        .unwrap();
        assert_eq!(answer.refusal().unwrap().reason, RefusalReason::Unknown);
    }

    #[test]
    fn an_unknown_answer_type_is_an_error() {
        let err = serde_json::from_value::<RfqAnswer>(json!({
            "v": 1, "type": "rfq_party", "rfq_id": "abc"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("rfq_party"), "{err}");
    }

    #[test]
    fn status_states_match_the_wire_and_the_terminal_set() {
        for (wire, state, terminal) in [
            ("quoted", RfqState::Quoted, false),
            ("funded", RfqState::Funded, false),
            ("filling", RfqState::Filling, false),
            ("filled", RfqState::Filled, false),
            ("settled", RfqState::Settled, true),
            ("refused", RfqState::Refused, true),
            ("expired", RfqState::Expired, true),
            ("refunded", RfqState::Refunded, true),
            ("stuck", RfqState::Stuck, true),
        ] {
            let status: RfqStatus = serde_json::from_value(json!({
                "v": 1, "type": "rfq_status", "rfq_id": "abc",
                "state": wire, "updated_at": 1, "profile": {}
            }))
            .unwrap();
            assert_eq!(status.state, state);
            assert_eq!(status.state.is_terminal(), terminal, "{wire}");
            assert_eq!(status.state.is_settled(), wire == "settled");
        }
    }

    #[test]
    fn fundable_needs_headroom_and_a_live_quote() {
        let quote: RfqQuote = serde_json::from_value(json!({
            "v": 1, "rfq_id": "abc", "pair": LIGHTNING_SEND_PAIR,
            "from_amount": 100, "to_amount": 99, "solver_pubkey": XONLY,
            "valid_until": 1_000_120, "refund_locktime": 1_000_000 + MIN_HEADROOM_SECONDS,
            "profile": {}
        }))
        .unwrap();
        quote
            .assert_fundable(1_000_000)
            .expect("exactly the headroom is enough");
        assert!(
            quote.assert_fundable(1_000_001).is_err(),
            "one second short"
        );
        assert!(quote.assert_fundable(1_000_120).is_err(), "quote expired");

        let mut no_deadline = quote.clone();
        no_deadline.refund_locktime = None;
        assert!(no_deadline.assert_fundable(1_000_000).is_err());
    }

    /// A signed BOLT11 for `payment_hash`, `amount_sats`, expiring `expiry`
    /// after `timestamp`.
    fn invoice(
        payment_hash: sha256::Hash,
        amount_sats: u64,
        timestamp: u64,
        expiry: u64,
    ) -> String {
        let secp = Secp256k1::new();
        let key = SecretKey::from_slice(&[0x42; 32]).unwrap();
        InvoiceBuilder::new(LnCurrency::Regtest)
            .description("corridor".into())
            .payment_hash(payment_hash)
            .payment_secret(PaymentSecret([7; 32]))
            .amount_milli_satoshis(amount_sats * 1_000)
            .duration_since_epoch(Duration::from_secs(timestamp))
            .expiry_time(Duration::from_secs(expiry))
            .min_final_cltv_expiry_delta(18)
            .build_signed(|msg: &Message| secp.sign_ecdsa_recoverable(msg, &key))
            .unwrap()
            .to_string()
    }

    fn receive_quote_with(invoice: &str, valid_until: u64, refund_locktime: u64) -> RfqQuote {
        let mut body = receive_quote_json();
        body["profile"]["invoice"] = json!(invoice);
        body["valid_until"] = json!(valid_until);
        body["refund_locktime"] = json!(refund_locktime);
        serde_json::from_value::<RfqAnswer>(body)
            .unwrap()
            .into_quote()
            .unwrap()
    }

    #[test]
    fn the_receive_invoice_must_pay_our_hash_for_the_quoted_amount() {
        let hash = sha256::Hash::hash(b"P");
        let issued = 1_800_000_000;
        let good = invoice(hash, 12121, issued, 600);
        let quote = receive_quote_with(&good, issued + 120, issued + 9_000);

        let facts = quote.verify_receive_invoice(&hash.to_string()).unwrap();
        assert_eq!(facts.amount_sats, 12121);
        assert_eq!(facts.invoice_expires_at, issued + 600);
        // The quote expires first, so that is the deadline.
        assert_eq!(facts.pay_deadline, issued + 120);

        // Upper-case hex is the same hash.
        quote
            .verify_receive_invoice(&hash.to_string().to_uppercase())
            .unwrap();

        let other = sha256::Hash::hash(b"Q");
        let err = quote
            .verify_receive_invoice(&other.to_string())
            .unwrap_err();
        assert!(err.message().contains("cannot claim"), "{err:?}");

        let wrong_amount = invoice(hash, 12000, issued, 600);
        let quote = receive_quote_with(&wrong_amount, issued + 120, issued + 9_000);
        let err = quote.verify_receive_invoice(&hash.to_string()).unwrap_err();
        assert!(err.message().contains("12000"), "{err:?}");
    }

    #[test]
    fn receivable_measures_the_claim_window_from_the_pay_deadline() {
        let hash = sha256::Hash::hash(b"P");
        let issued = 1_800_000_000;
        let inv = invoice(hash, 12121, issued, 3_600);
        // Quote outlives the invoice, so the invoice's expiry is the deadline.
        let quote = receive_quote_with(
            &inv,
            issued + 7_200,
            issued + 3_600 + MIN_CLAIM_WINDOW_SECONDS,
        );
        let facts = quote.verify_receive_invoice(&hash.to_string()).unwrap();
        assert_eq!(facts.pay_deadline, issued + 3_600);

        quote
            .assert_receivable(&facts, issued, None)
            .expect("exactly the window");
        quote
            .assert_receivable(&facts, issued, Some(12121))
            .expect("at the ceiling");
        assert!(quote
            .assert_receivable(&facts, issued, Some(12120))
            .is_err());
        assert!(
            quote
                .assert_receivable(&facts, issued + 3_600, None)
                .is_err(),
            "deadline passed"
        );

        let tight = receive_quote_with(
            &inv,
            issued + 7_200,
            issued + 3_600 + MIN_CLAIM_WINDOW_SECONDS - 1,
        );
        let facts = tight.verify_receive_invoice(&hash.to_string()).unwrap();
        assert!(tight.assert_receivable(&facts, issued, None).is_err());
    }

    #[test]
    fn the_corridor_root_is_the_origin_the_v2_base_hangs_off() {
        for (base, root) in [
            (
                "https://maker.signet.kaleidoswap.com/v2",
                "https://maker.signet.kaleidoswap.com",
            ),
            (
                "https://maker.signet.kaleidoswap.com/v2/",
                "https://maker.signet.kaleidoswap.com",
            ),
            ("http://localhost:9001/v2", "http://localhost:9001"),
            ("https://host/prefix/v2", "https://host/prefix"),
        ] {
            assert_eq!(corridor_root_from_maker_url(base).unwrap(), root, "{base}");
        }
        for bad in [
            "https://maker.example",
            "https://maker.example/v1",
            "https://maker.example/v2?x=1",
            "https://maker.example/v2#frag",
            "not a url",
        ] {
            assert!(corridor_root_from_maker_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rfq_ids_are_32_random_bytes_of_hex() {
        let a = new_rfq_id();
        let b = new_rfq_id();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
