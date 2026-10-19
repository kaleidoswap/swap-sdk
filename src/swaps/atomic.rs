//! BTC L1 <-> USDT-RGB L1 atomic swaps — the maker's `/v2/swap/atomic` wire,
//! typed.
//!
//! These are not Boltz chain swaps. The maker and the taker build one
//! transaction that moves both legs: no HTLC, no timelock, no refund. Either
//! the transaction confirms and both sides are paid, or it never reaches the
//! chain and neither is.
//!
//! The flow, and where each step runs:
//!
//! 1. [`BoltzApiClientV2::atomic_quote`] prices the swap and returns
//!    rgb-lib's `OnchainSwapOffer`. [`AtomicQuote::verify`] checks the quote
//!    against what was asked for, and the offer against the quote, before
//!    the offer reaches a wallet.
//! 2. The **wallet** runs rgb-lib's `accept_swap_offer` on the offer.
//!    [`BoltzApiClientV2::atomic_request`] posts the resulting
//!    `OnchainSwapRequest` and returns the maker's unsigned
//!    `OnchainSwapProposal`.
//! 3. The **wallet** signs the proposal with `complete_swap_proposal`.
//!    [`BoltzApiClientV2::atomic_complete`] posts the `OnchainSwapCompletion`;
//!    the maker signs last and broadcasts in the same call.
//! 4. [`BoltzApiClientV2::atomic_swap`] reports where the swap stands. Poll
//!    until [`AtomicSwapState::is_terminal`], and accept the incoming
//!    transfer in the wallet with rgb-lib's `accept_swap_transfers`.
//!
//! The SDK does not bundle rgb-lib. The offer, request, proposal and
//! completion travel as JSON values; only the fields checked here are read.
//!
//! ## What rgb-lib checks, and what it cannot
//!
//! `complete_swap_proposal` signs only a proposal built from the request the
//! taker's own wallet saved, and it rebuilds the PSBT to compare outputs and
//! the miner fee. It cannot know the price the user agreed to: the offer is
//! its only source of terms. [`AtomicQuote::verify`] closes that gap by tying
//! the offer to the quote and the quote to the request. It does not check
//! that the proposal's maker inputs are not the taker's own UTXOs; that needs
//! the wallet.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;
use crate::swaps::boltz::BoltzApiClientV2;

/// The pair that sends BTC and receives USDT-RGB.
pub const ATOMIC_BTC_TO_USDT_RGB: &str = "BTC/USDT-RGB";
/// The pair that sends USDT-RGB and receives BTC.
pub const ATOMIC_USDT_RGB_TO_BTC: &str = "USDT-RGB/BTC";

/// Which amount of a quote request is fixed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtomicAmountDirection {
    /// `amount` is what the taker sends.
    #[default]
    From,
    /// `amount` is what the taker receives.
    To,
}

/// `POST /v2/swap/atomic/quote`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicQuoteRequest {
    /// [`ATOMIC_BTC_TO_USDT_RGB`] or [`ATOMIC_USDT_RGB_TO_BTC`].
    pub pair: String,
    /// Sats for BTC, the RGB contract's smallest unit for the asset.
    pub amount: u64,
    #[serde(default)]
    pub direction: AtomicAmountDirection,
}

/// The maker's quote, with rgb-lib's offer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicQuote {
    pub id: String,
    pub pair: String,
    pub direction: AtomicAmountDirection,
    /// What the taker sends.
    pub from_amount: u64,
    /// What the taker receives, net of the maker's fee.
    pub to_amount: u64,
    /// The RGB contract of the RGB leg.
    pub asset_id: String,
    /// Miner fee the taker funds on top of `from_amount`, sats.
    pub network_fee_sat: u64,
    /// The maker's fee, in the output asset's units.
    pub service_fee: u64,
    /// Unix seconds: the request must arrive before this.
    pub expires_at: i64,
    /// Unix seconds: the completion must arrive before this.
    pub offer_expires_at: i64,
    /// rgb-lib's `OnchainSwapOffer`, for the wallet's `accept_swap_offer`.
    pub offer: Value,
}

/// What the caller expects of a quote, beyond what it asked for.
///
/// Amounts are in each asset's base units, the ones the quote itself uses:
/// sats for BTC, and the RGB contract's smallest unit for USDT-RGB, which has
/// 6 decimals (1 USDT = 1_000_000). Never the pair card's display units.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicQuoteExpectations {
    /// The USDT-RGB contract id the wallet trades. Required: a quote for
    /// another contract is a different asset.
    pub asset_id: String,
    /// rgb-lib's `BitcoinNetwork` for the wallet, e.g. `"Regtest"` or
    /// `"Signet"`. The offer must name the same one.
    pub bitcoin_network: String,
    /// RGB proxy the wallet uses, if it requires a specific one.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Ceiling on the miner fee the taker funds, sats.
    #[serde(default)]
    pub max_network_fee_sat: Option<u64>,
    /// Ceiling on the fee the maker *declares* (`serviceFee`), in the output
    /// asset's base units: sats when the taker receives BTC, the RGB
    /// contract's smallest unit when it receives USDT-RGB.
    ///
    /// This is not price protection. A maker can declare no fee and take its
    /// cut as a worse rate in the quoted amounts; only `min_to_amount` (for a
    /// `from` quote) or `max_from_amount` (for a `to` quote) bounds what the
    /// taker actually gets or pays.
    #[serde(default)]
    pub max_service_fee: Option<u64>,
    /// For a `from` quote: the least the taker accepts to receive, in the
    /// output asset's base units. Together with `max_from_amount`, the only
    /// bound on the price.
    #[serde(default)]
    pub min_to_amount: Option<u64>,
    /// For a `to` quote: the most the taker accepts to send, in the input
    /// asset's base units.
    #[serde(default)]
    pub max_from_amount: Option<u64>,
    /// Ceiling on `rgb_output_sat`: the sats each RGB receive output carries.
    /// The taker funds it into its own coloured UTXO, so nothing is lost, but
    /// without a ceiling a maker could tie up any amount of the taker's BTC
    /// there.
    #[serde(default)]
    pub max_rgb_output_sat: Option<u64>,
    /// Longest the offer may stay open, seconds from now. A long-lived offer
    /// is an option on the price held by the maker.
    #[serde(default)]
    pub max_offer_ttl_secs: Option<u64>,
}

/// The kind of one leg of an rgb-lib offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LegKind {
    Btc,
    Rgb,
}

/// One leg of an rgb-lib offer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct OfferLeg {
    kind: LegKind,
    asset_id: Option<String>,
    #[serde(deserialize_with = "u64_from_str_or_number")]
    amount: u64,
}

impl fmt::Display for OfferLeg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.kind, &self.asset_id) {
            (LegKind::Btc, _) => write!(f, "{} sat BTC", self.amount),
            (LegKind::Rgb, Some(asset)) => write!(f, "{} units of {asset}", self.amount),
            (LegKind::Rgb, None) => write!(f, "{} units of an unnamed RGB asset", self.amount),
        }
    }
}

/// The fields of rgb-lib's `OnchainSwapOffer` that carry the terms. rgb-lib
/// writes them in snake case, numbers as numbers or strings.
#[derive(Debug, Deserialize)]
struct OfferTerms {
    swap_id: String,
    maker_gives: OfferLeg,
    maker_receives: OfferLeg,
    bitcoin_network: String,
    #[serde(deserialize_with = "u64_from_str_or_number")]
    network_fee_sat: u64,
    #[serde(default, deserialize_with = "opt_u64_from_str_or_number")]
    rgb_output_sat: Option<u64>,
    #[serde(default, deserialize_with = "opt_u64_from_str_or_number")]
    expiration_timestamp: Option<u64>,
    #[serde(default)]
    maker_btc_address: Option<String>,
    #[serde(default)]
    maker_rgb_recipient_id: Option<String>,
    #[serde(default)]
    proxy_url: Option<String>,
    #[serde(default, deserialize_with = "opt_u64_from_str_or_number")]
    platform_fee_sat: Option<u64>,
    #[serde(default)]
    fee_recipient_script_pubkey_hex: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StrOrU64 {
    Number(u64),
    Str(String),
}

impl StrOrU64 {
    fn into_u64<E: de::Error>(self) -> Result<u64, E> {
        match self {
            Self::Number(n) => Ok(n),
            Self::Str(s) => u64::from_str(&s).map_err(E::custom),
        }
    }
}

fn u64_from_str_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    StrOrU64::deserialize(d)?.into_u64()
}

fn opt_u64_from_str_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Option::<StrOrU64>::deserialize(d)?
        .map(StrOrU64::into_u64)
        .transpose()
}

fn mismatch(quote: &AtomicQuote, what: impl fmt::Display) -> Error {
    Error::Protocol(format!("atomic quote {}: {what}", quote.id))
}

impl AtomicQuote {
    /// Check the quote before its offer goes to the wallet.
    ///
    /// Refuses a quote that answers a different pair, direction or amount
    /// than `request`; one for another asset, network or proxy than
    /// `expect`; one over any of `expect`'s ceilings; one whose request
    /// deadline has passed at `now` (unix seconds); and an offer whose legs,
    /// miner fee or expiry differ from the quote, or that adds a platform
    /// fee. The wallet signs the offer, not the quote, so the offer is what
    /// has to match.
    pub fn verify(
        &self,
        request: &AtomicQuoteRequest,
        expect: &AtomicQuoteExpectations,
        now: u64,
    ) -> Result<(), Error> {
        if self.pair != request.pair {
            return Err(mismatch(
                self,
                format!("is for pair {}, not {}", self.pair, request.pair),
            ));
        }
        let taker_sends_btc = match self.pair.as_str() {
            ATOMIC_BTC_TO_USDT_RGB => true,
            ATOMIC_USDT_RGB_TO_BTC => false,
            other => return Err(mismatch(self, format!("unknown atomic pair {other}"))),
        };
        if self.direction != request.direction {
            return Err(mismatch(
                self,
                format!(
                    "fixes the {:?} amount, the request fixed {:?}",
                    self.direction, request.direction
                ),
            ));
        }
        let fixed = match request.direction {
            AtomicAmountDirection::From => self.from_amount,
            AtomicAmountDirection::To => self.to_amount,
        };
        if fixed != request.amount {
            return Err(mismatch(
                self,
                format!(
                    "quotes {fixed} for the fixed amount, the request asked {}",
                    request.amount
                ),
            ));
        }
        if self.from_amount == 0 || self.to_amount == 0 {
            return Err(mismatch(self, "has a zero amount"));
        }
        if self.asset_id != expect.asset_id {
            return Err(mismatch(
                self,
                format!(
                    "is for asset {}, the wallet trades {}",
                    self.asset_id, expect.asset_id
                ),
            ));
        }
        if let Some(max) = expect.max_network_fee_sat {
            if self.network_fee_sat > max {
                return Err(mismatch(
                    self,
                    format!(
                        "asks a {} sat miner fee, over the {max} sat ceiling",
                        self.network_fee_sat
                    ),
                ));
            }
        }
        if let Some(max) = expect.max_service_fee {
            if self.service_fee > max {
                return Err(mismatch(
                    self,
                    format!(
                        "charges a {} service fee, over the {max} ceiling",
                        self.service_fee
                    ),
                ));
            }
        }
        if let (AtomicAmountDirection::From, Some(min)) = (self.direction, expect.min_to_amount) {
            if self.to_amount < min {
                return Err(mismatch(
                    self,
                    format!("pays {}, under the {min} minimum", self.to_amount),
                ));
            }
        }
        if let (AtomicAmountDirection::To, Some(max)) = (self.direction, expect.max_from_amount) {
            if self.from_amount > max {
                return Err(mismatch(
                    self,
                    format!("costs {}, over the {max} maximum", self.from_amount),
                ));
            }
        }
        let now_i = i64::try_from(now).unwrap_or(i64::MAX);
        if now_i >= self.expires_at {
            return Err(mismatch(
                self,
                format!("expired at {} (now {now})", self.expires_at),
            ));
        }
        if self.offer_expires_at < self.expires_at {
            return Err(mismatch(
                self,
                format!(
                    "offer expires at {}, before the request deadline {}",
                    self.offer_expires_at, self.expires_at
                ),
            ));
        }
        if let Some(max_ttl) = expect.max_offer_ttl_secs {
            let ttl = u64::try_from(self.offer_expires_at - now_i).unwrap_or(u64::MAX);
            if ttl > max_ttl {
                return Err(mismatch(
                    self,
                    format!("offer stays open {ttl}s, over the {max_ttl}s maximum"),
                ));
            }
        }
        self.verify_offer(taker_sends_btc, expect)
    }

    fn verify_offer(
        &self,
        taker_sends_btc: bool,
        expect: &AtomicQuoteExpectations,
    ) -> Result<(), Error> {
        let offer: OfferTerms = serde_json::from_value(self.offer.clone())
            .map_err(|e| mismatch(self, format!("offer is not an rgb-lib swap offer: {e}")))?;
        if offer.swap_id.is_empty() {
            return Err(mismatch(self, "offer has no swap_id"));
        }
        let btc = |amount| OfferLeg {
            kind: LegKind::Btc,
            asset_id: None,
            amount,
        };
        let rgb = |amount| OfferLeg {
            kind: LegKind::Rgb,
            asset_id: Some(self.asset_id.clone()),
            amount,
        };
        let (gives, receives) = if taker_sends_btc {
            (rgb(self.to_amount), btc(self.from_amount))
        } else {
            (btc(self.to_amount), rgb(self.from_amount))
        };
        if offer.maker_gives != gives {
            return Err(mismatch(
                self,
                format!("offer gives {}, the quote {gives}", offer.maker_gives),
            ));
        }
        if offer.maker_receives != receives {
            return Err(mismatch(
                self,
                format!("offer takes {}, the quote {receives}", offer.maker_receives),
            ));
        }
        if offer.network_fee_sat != self.network_fee_sat {
            return Err(mismatch(
                self,
                format!(
                    "offer reserves a {} sat miner fee, the quote {}",
                    offer.network_fee_sat, self.network_fee_sat
                ),
            ));
        }
        if let Some(max) = expect.max_rgb_output_sat {
            let output = offer
                .rgb_output_sat
                .ok_or_else(|| mismatch(self, "offer does not say what its RGB output carries"))?;
            if output > max {
                return Err(mismatch(
                    self,
                    format!("offer puts {output} sat in each RGB output, more than {max}"),
                ));
            }
        }
        if offer.platform_fee_sat.unwrap_or(0) != 0
            || offer.fee_recipient_script_pubkey_hex.is_some()
        {
            return Err(mismatch(
                self,
                "offer adds a platform fee the quote does not show",
            ));
        }
        let offer_expiry = offer
            .expiration_timestamp
            .and_then(|t| i64::try_from(t).ok());
        if offer_expiry != Some(self.offer_expires_at) {
            return Err(mismatch(
                self,
                format!(
                    "offer expires at {:?}, the quote at {}",
                    offer.expiration_timestamp, self.offer_expires_at
                ),
            ));
        }
        if offer.bitcoin_network != expect.bitcoin_network {
            return Err(mismatch(
                self,
                format!(
                    "offer is for {}, the wallet is on {}",
                    offer.bitcoin_network, expect.bitcoin_network
                ),
            ));
        }
        if let Some(proxy) = &expect.proxy_url {
            if offer.proxy_url.as_ref() != Some(proxy) {
                return Err(mismatch(
                    self,
                    format!("offer uses proxy {:?}, the wallet {proxy}", offer.proxy_url),
                ));
            }
        }
        let maker_receive_named = if taker_sends_btc {
            offer.maker_btc_address.is_some()
        } else {
            offer.maker_rgb_recipient_id.is_some()
        };
        if !maker_receive_named {
            return Err(mismatch(self, "offer names no maker receive destination"));
        }
        Ok(())
    }
}

/// A coin an rgb-lib swap message spends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AtomicOutpoint {
    pub txid: String,
    pub vout: u32,
}

impl fmt::Display for AtomicOutpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.txid, self.vout)
    }
}

/// `POST /v2/swap/atomic/{id}/request` — the maker's unsigned proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicProposal {
    pub id: String,
    pub status: String,
    /// rgb-lib's `OnchainSwapProposal`, for the wallet's
    /// `complete_swap_proposal`.
    pub proposal: Value,
    /// The coins the proposal says are the maker's (`proposal.maker_inputs`).
    /// None of them is one of the wallet outpoints given to
    /// [`BoltzApiClientV2::atomic_request`], and none repeats.
    #[serde(rename = "makerInputs", default, skip_deserializing)]
    pub maker_inputs: Vec<AtomicOutpoint>,
}

/// `POST /v2/swap/atomic/{id}/complete` — the broadcast swap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicCompletion {
    pub id: String,
    pub status: String,
    pub txid: String,
    /// The finalized `OnchainSwapCompletion`, for the wallet's
    /// `accept_swap_transfers`.
    pub completion: Value,
}

/// Where an atomic swap stands, by its wire status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomicSwapState {
    #[serde(rename = "swap.created")]
    Created,
    #[serde(rename = "swap.requested")]
    Requested,
    #[serde(rename = "transaction.proposed")]
    Proposed,
    #[serde(rename = "transaction.mempool")]
    Mempool,
    #[serde(rename = "transaction.confirmed")]
    Confirmed,
    #[serde(rename = "transaction.claimed")]
    Claimed,
    #[serde(rename = "swap.expired")]
    Expired,
    #[serde(rename = "swap.cancelled")]
    Cancelled,
    #[serde(rename = "transaction.failed")]
    Failed,
}

impl AtomicSwapState {
    /// No further update follows one of these. Poll until one.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Claimed | Self::Expired | Self::Cancelled | Self::Failed
        )
    }

    /// The one terminal state that is a success: the maker has accepted its
    /// side of the swap transaction.
    pub fn is_settled(self) -> bool {
        matches!(self, Self::Claimed)
    }
}

/// `GET /v2/swap/atomic/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicSwapStatus {
    pub id: String,
    pub pair: String,
    pub status: AtomicSwapState,
    /// The maker's internal state (`quoted` … `settled`), for diagnostics.
    pub state: String,
    pub from_amount: u64,
    pub to_amount: u64,
    pub asset_id: String,
    pub network_fee_sat: u64,
    #[serde(default)]
    pub txid: Option<String>,
    pub confirmations: u32,
    pub expires_at: i64,
    pub offer_expires_at: i64,
    pub created_at: i64,
}

/// One atomic pair's rate card. Atomic cards carry no zero-conf limit and a
/// flat `minerFees`, which the per-swap quote replaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPair {
    pub hash: String,
    pub rate: f64,
    pub limits: AtomicPairLimits,
    pub fees: AtomicPairFees,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicPairLimits {
    pub minimal: u64,
    pub maximal: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPairFees {
    pub percentage: f64,
    pub miner_fees: u64,
}

/// `GET /v2/swap/atomic/pairs`: `from` currency → `to` currency → card. A
/// halted maker answers an empty map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GetAtomicPairsResponse {
    pub pairs: HashMap<String, HashMap<String, AtomicPair>>,
}

impl GetAtomicPairsResponse {
    pub fn get(&self, from: &str, to: &str) -> Option<&AtomicPair> {
        self.pairs.get(from)?.get(to)
    }
}

/// Check the proposal answers the request that was posted: rgb-lib embeds
/// the request in the proposal, and the wallet only signs a proposal for the
/// request it saved, so a mismatch here is a proposal it will refuse.
/// The outpoints of an rgb-lib input list (`[{ "outpoint": { "txid", "vout" } }]`).
fn input_outpoints(inputs: Option<&Value>, what: &str) -> Result<Vec<AtomicOutpoint>, Error> {
    let malformed = || Error::Protocol(format!("atomic swap {what} is not a list of inputs"));
    let Some(inputs) = inputs else {
        return Ok(Vec::new());
    };
    inputs
        .as_array()
        .ok_or_else(malformed)?
        .iter()
        .map(|input| {
            let outpoint = input.get("outpoint").ok_or_else(malformed)?;
            let txid = outpoint
                .get("txid")
                .and_then(Value::as_str)
                .and_then(|txid| bitcoin::Txid::from_str(txid).ok())
                .ok_or_else(malformed)?;
            let vout = match outpoint.get("vout") {
                Some(Value::Number(n)) => n.as_u64(),
                Some(Value::String(s)) => s.parse().ok(),
                _ => None,
            }
            .and_then(|vout| u32::try_from(vout).ok())
            .ok_or_else(malformed)?;
            // The canonical form: lowercase hex, whatever the message used.
            Ok(AtomicOutpoint {
                txid: txid.to_string(),
                vout,
            })
        })
        .collect()
}

/// An outpoint as compared: the parsed txid, so that two spellings of one
/// coin (e.g. upper- and lowercase hex) are the same key.
fn outpoint_key(outpoint: &AtomicOutpoint) -> Option<(bitcoin::Txid, u32)> {
    Some((bitcoin::Txid::from_str(&outpoint.txid).ok()?, outpoint.vout))
}

/// The wallet's outpoints as comparison keys.
fn wallet_keys(wallet: &[AtomicOutpoint]) -> Result<HashSet<(bitcoin::Txid, u32)>, Error> {
    wallet
        .iter()
        .map(|outpoint| {
            outpoint_key(outpoint).ok_or_else(|| {
                Error::Generic(format!("wallet outpoint {outpoint} does not name a txid"))
            })
        })
        .collect()
}

/// Check `wallet` can be the taker's whole wallet: it must hold the request's
/// own `taker_inputs`, which catches an empty or partial list passed by
/// mistake before anything is sent.
pub fn check_wallet_outpoints(request: &Value, wallet: &[AtomicOutpoint]) -> Result<(), Error> {
    let wallet = wallet_keys(wallet)?;
    let requested = input_outpoints(request.get("taker_inputs"), "request's taker_inputs")?;
    if let Some(missing) = requested
        .iter()
        .find(|outpoint| outpoint_key(outpoint).is_none_or(|key| !wallet.contains(&key)))
    {
        return Err(Error::Generic(format!(
            "wallet outpoints must be every outpoint the wallet holds (list_unspents); \
             they lack the request's own input {missing}"
        )));
    }
    Ok(())
}

/// Check the proposal answers the request that was posted, and lists none of
/// the taker's coins as the maker's. Returns the maker's inputs.
///
/// `wallet` is every outpoint the taker's wallet holds (rgb-lib's
/// `list_unspents`). It must contain the request's own `taker_inputs`, which
/// catches an empty or partial list passed by mistake.
pub(crate) fn check_proposal(
    id: &str,
    request: &Value,
    answer: &AtomicProposal,
    wallet: &[AtomicOutpoint],
) -> Result<Vec<AtomicOutpoint>, Error> {
    if answer.id != id {
        return Err(Error::Protocol(format!(
            "atomic swap {id}: proposal answers swap {}",
            answer.id
        )));
    }
    if answer.proposal.get("request") != Some(request) {
        return Err(Error::Protocol(format!(
            "atomic swap {id}: proposal is not for the request that was sent"
        )));
    }
    check_wallet_outpoints(request, wallet)?;
    let wallet = wallet_keys(wallet)?;
    let maker = input_outpoints(
        answer.proposal.get("maker_inputs"),
        "proposal's maker_inputs",
    )?;
    let mut seen = HashSet::new();
    for outpoint in &maker {
        // `input_outpoints` parsed every txid, so this key exists.
        let key = outpoint_key(outpoint).ok_or_else(|| {
            Error::Protocol(format!(
                "atomic swap {id}: maker input {outpoint} is not a txid"
            ))
        })?;
        if wallet.contains(&key) {
            return Err(Error::Protocol(format!(
                "atomic swap {id}: proposal lists the taker's own coin {outpoint} as the maker's"
            )));
        }
        if !seen.insert(key) {
            return Err(Error::Protocol(format!(
                "atomic swap {id}: proposal lists the maker's input {outpoint} twice"
            )));
        }
    }
    Ok(maker)
}

/// Check the broadcast answers the completion that was posted.
pub(crate) fn check_completion(
    id: &str,
    completion: &Value,
    answer: &AtomicCompletion,
) -> Result<(), Error> {
    if answer.id != id {
        return Err(Error::Protocol(format!(
            "atomic swap {id}: completion answers swap {}",
            answer.id
        )));
    }
    let sent_txid = completion.get("txid").and_then(Value::as_str);
    if sent_txid != Some(answer.txid.as_str()) {
        return Err(Error::Protocol(format!(
            "atomic swap {id}: maker broadcast {}, the signed completion is {}",
            answer.txid,
            sent_txid.unwrap_or("missing")
        )));
    }
    if answer.completion.get("proposal") != completion.get("proposal") {
        return Err(Error::Protocol(format!(
            "atomic swap {id}: finalized completion is for another proposal"
        )));
    }
    Ok(())
}

// Atomic swaps are not part of the Boltz protocol, so their routes live here
// rather than in `boltz.rs`, which tracks upstream boltz-rust.
impl BoltzApiClientV2 {
    /// `GET /v2/swap/atomic/pairs` — the atomic rate cards.
    pub async fn atomic_pairs(&self) -> Result<GetAtomicPairsResponse, Error> {
        self.get_json("swap/atomic/pairs").await
    }

    /// `POST /v2/swap/atomic/quote`, checked with
    /// [`AtomicQuote::verify`] before it is returned.
    ///
    /// `now` is unix seconds; the caller supplies it because the browser
    /// build has no system clock.
    pub async fn atomic_quote(
        &self,
        request: &AtomicQuoteRequest,
        expect: &AtomicQuoteExpectations,
        now: u64,
    ) -> Result<AtomicQuote, Error> {
        let quote: AtomicQuote = self.post_json("swap/atomic/quote", request).await?;
        quote.verify(request, expect, now)?;
        Ok(quote)
    }

    /// `POST /v2/swap/atomic/{id}/request` with the wallet's
    /// `OnchainSwapRequest`. Refuses a proposal for another swap or request.
    ///
    /// `wallet_outpoints` is every outpoint the taker's wallet holds
    /// (rgb-lib's `list_unspents`). A proposal that lists any of them as the
    /// maker's is refused: rgb-lib before kaleidoswap/rgb-lib#18 would sign
    /// such a coin and hand its value to the maker.
    pub async fn atomic_request(
        &self,
        id: &str,
        request: &Value,
        wallet_outpoints: &[AtomicOutpoint],
    ) -> Result<AtomicProposal, Error> {
        check_wallet_outpoints(request, wallet_outpoints)?;
        let mut answer: AtomicProposal = self
            .post_json(
                &format!("swap/atomic/{id}/request"),
                serde_json::json!({ "request": request }),
            )
            .await?;
        answer.maker_inputs = check_proposal(id, request, &answer, wallet_outpoints)?;
        Ok(answer)
    }

    /// `POST /v2/swap/atomic/{id}/complete` with the wallet's signed
    /// `OnchainSwapCompletion`. The maker signs last and broadcasts; this
    /// refuses an answer whose txid or proposal is not the one signed.
    pub async fn atomic_complete(
        &self,
        id: &str,
        completion: &Value,
    ) -> Result<AtomicCompletion, Error> {
        let answer = self
            .post_json(
                &format!("swap/atomic/{id}/complete"),
                serde_json::json!({ "completion": completion }),
            )
            .await?;
        check_completion(id, completion, &answer)?;
        Ok(answer)
    }

    /// `GET /v2/swap/atomic/{id}` — where an atomic swap stands. Poll until
    /// [`AtomicSwapState::is_terminal`].
    pub async fn atomic_swap(&self, id: &str) -> Result<AtomicSwapStatus, Error> {
        self.get_json(&format!("swap/atomic/{id}")).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ASSET: &str = "rgb:test-usdt-contract";
    const NOW: u64 = 1_800_000_000;

    fn request() -> AtomicQuoteRequest {
        AtomicQuoteRequest {
            pair: ATOMIC_BTC_TO_USDT_RGB.to_string(),
            amount: 100_000,
            direction: AtomicAmountDirection::From,
        }
    }

    fn expect() -> AtomicQuoteExpectations {
        AtomicQuoteExpectations {
            asset_id: ASSET.to_string(),
            bitcoin_network: "Regtest".to_string(),
            ..Default::default()
        }
    }

    /// A `BTC/USDT-RGB` quote as the maker sends it: the maker gives
    /// 95,000,000 RGB units and receives 100,000 sat.
    fn quote() -> AtomicQuote {
        serde_json::from_value(json!({
            "id": "01JATOMICSWAP",
            "pair": "BTC/USDT-RGB",
            "direction": "from",
            "fromAmount": 100_000,
            "toAmount": 95_000_000,
            "assetId": ASSET,
            "networkFeeSat": 1_200,
            "serviceFee": 500_000,
            "expiresAt": NOW + 60,
            "offerExpiresAt": NOW + 300,
            "offer": {
                "swap_id": "rgbswap1",
                "maker_gives": { "kind": "Rgb", "asset_id": ASSET, "amount": 95_000_000 },
                "maker_receives": { "kind": "Btc", "asset_id": null, "amount": "100000" },
                "bitcoin_network": "Regtest",
                "network_fee_sat": "1200",
                "rgb_output_sat": 1000,
                "expiration_timestamp": NOW + 300,
                "maker_btc_address": "bcrt1qmaker",
                "proxy_url": "rpcs://proxy.example/json-rpc",
                "platform_fee_sat": 0,
                "fee_recipient_script_pubkey_hex": null
            }
        }))
        .unwrap()
    }

    fn refused(quote: &AtomicQuote, expect: &AtomicQuoteExpectations) -> String {
        quote
            .verify(&request(), expect, NOW)
            .expect_err("the quote must be refused")
            .to_string()
    }

    #[test]
    fn a_matching_quote_is_accepted() {
        quote().verify(&request(), &expect(), NOW).unwrap();
    }

    #[test]
    fn the_reverse_pair_mirrors_the_offer_legs() {
        let mut q = quote();
        q.pair = ATOMIC_USDT_RGB_TO_BTC.to_string();
        q.from_amount = 95_000_000;
        q.to_amount = 100_000;
        q.offer["maker_gives"] = json!({ "kind": "Btc", "asset_id": null, "amount": 100_000 });
        q.offer["maker_receives"] =
            json!({ "kind": "Rgb", "asset_id": ASSET, "amount": 95_000_000 });
        q.offer["maker_btc_address"] = Value::Null;
        q.offer["maker_rgb_recipient_id"] = json!("rgb:invoice-recipient");
        let req = AtomicQuoteRequest {
            pair: ATOMIC_USDT_RGB_TO_BTC.to_string(),
            amount: 95_000_000,
            direction: AtomicAmountDirection::From,
        };
        q.verify(&req, &expect(), NOW).unwrap();

        // The same legs under the other pair's label are refused.
        let mut swapped = quote();
        swapped.offer["maker_gives"] = q.offer["maker_gives"].clone();
        swapped.offer["maker_receives"] = q.offer["maker_receives"].clone();
        assert!(refused(&swapped, &expect()).contains("offer gives"));
    }

    #[test]
    fn the_quote_must_answer_the_request() {
        let mut q = quote();
        q.pair = ATOMIC_USDT_RGB_TO_BTC.to_string();
        assert!(refused(&q, &expect()).contains("is for pair"));

        let mut q = quote();
        q.direction = AtomicAmountDirection::To;
        assert!(refused(&q, &expect()).contains("fixes the"));

        let mut q = quote();
        q.from_amount = 99_999;
        assert!(refused(&q, &expect()).contains("fixed amount"));
    }

    #[test]
    fn another_asset_or_network_is_refused() {
        let mut q = quote();
        q.asset_id = "rgb:other".to_string();
        assert!(refused(&q, &expect()).contains("is for asset"));

        let mut e = expect();
        e.bitcoin_network = "Signet".to_string();
        assert!(refused(&quote(), &e).contains("offer is for Regtest"));

        let mut e = expect();
        e.proxy_url = Some("rpcs://other.example/json-rpc".to_string());
        assert!(refused(&quote(), &e).contains("proxy"));
    }

    #[test]
    fn ceilings_are_enforced() {
        let mut e = expect();
        e.max_network_fee_sat = Some(1_199);
        assert!(refused(&quote(), &e).contains("miner fee"));

        let mut e = expect();
        e.max_service_fee = Some(499_999);
        assert!(refused(&quote(), &e).contains("service fee"));

        let mut e = expect();
        e.min_to_amount = Some(95_000_001);
        assert!(refused(&quote(), &e).contains("minimum"));

        let mut e = expect();
        e.max_offer_ttl_secs = Some(299);
        assert!(refused(&quote(), &e).contains("stays open"));

        let mut e = expect();
        e.max_network_fee_sat = Some(1_200);
        e.max_service_fee = Some(500_000);
        e.min_to_amount = Some(95_000_000);
        e.max_offer_ttl_secs = Some(300);
        quote().verify(&request(), &e, NOW).unwrap();
    }

    #[test]
    fn an_rgb_output_above_the_ceiling_is_refused() {
        let mut e = expect();
        e.max_rgb_output_sat = Some(1_000);
        quote().verify(&request(), &e, NOW).unwrap();

        e.max_rgb_output_sat = Some(999);
        assert!(refused(&quote(), &e).contains("1000 sat in each RGB output"));

        let mut q = quote();
        q.offer.as_object_mut().unwrap().remove("rgb_output_sat");
        assert!(refused(&q, &e).contains("does not say what its RGB output carries"));
        // Without a ceiling the field is not required.
        q.verify(&request(), &expect(), NOW).unwrap();
    }

    #[test]
    fn an_expired_quote_is_refused() {
        let q = quote();
        let err = q
            .verify(&request(), &expect(), NOW + 60)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expired"), "{err}");
    }

    #[test]
    fn the_offer_must_carry_the_quoted_terms() {
        let mut q = quote();
        q.offer["maker_gives"]["amount"] = json!(94_999_999);
        assert!(refused(&q, &expect()).contains("offer gives"));

        let mut q = quote();
        q.offer["maker_gives"]["asset_id"] = json!("rgb:other");
        assert!(refused(&q, &expect()).contains("offer gives"));

        let mut q = quote();
        q.offer["maker_receives"]["amount"] = json!(100_001);
        assert!(refused(&q, &expect()).contains("offer takes"));

        let mut q = quote();
        q.offer["network_fee_sat"] = json!(5_000);
        assert!(refused(&q, &expect()).contains("miner fee"));

        let mut q = quote();
        q.offer["platform_fee_sat"] = json!(1_000);
        q.offer["fee_recipient_script_pubkey_hex"] = json!("0014aaaa");
        assert!(refused(&q, &expect()).contains("platform fee"));

        let mut q = quote();
        q.offer["expiration_timestamp"] = json!(NOW + 86_400);
        assert!(refused(&q, &expect()).contains("offer expires"));

        let mut q = quote();
        q.offer["maker_btc_address"] = Value::Null;
        assert!(refused(&q, &expect()).contains("receive destination"));

        let mut q = quote();
        q.offer = json!({ "swap_id": "x" });
        assert!(refused(&q, &expect()).contains("not an rgb-lib swap offer"));
    }

    #[test]
    fn a_proposal_must_answer_the_sent_request() {
        let request = json!({ "offer": { "swap_id": "rgbswap1" }, "taker_inputs": [] });
        let answer = AtomicProposal {
            id: "01JATOMICSWAP".to_string(),
            status: "transaction.proposed".to_string(),
            proposal: json!({ "request": request, "psbt": "cHNidP8=" }),
            maker_inputs: vec![],
        };
        check_proposal("01JATOMICSWAP", &request, &answer, &[]).unwrap();
        assert!(check_proposal("01JOTHER", &request, &answer, &[]).is_err());
        let other = json!({ "offer": { "swap_id": "rgbswap2" }, "taker_inputs": [] });
        assert!(check_proposal("01JATOMICSWAP", &other, &answer, &[]).is_err());
    }

    fn outpoint(txid: &str, vout: u32) -> AtomicOutpoint {
        AtomicOutpoint {
            txid: txid.repeat(32),
            vout,
        }
    }

    fn input(outpoint: &AtomicOutpoint, vout: Value) -> Value {
        json!({ "outpoint": { "txid": outpoint.txid, "vout": vout }, "amount_sat": 1000 })
    }

    /// A request spending the taker's `aa:0`, from a wallet that also holds
    /// `bb:1` and `dd:5`.
    fn taker_side() -> (Value, Vec<AtomicOutpoint>) {
        let spent = outpoint("aa", 0);
        let request = json!({
            "offer": { "swap_id": "rgbswap1" },
            "taker_inputs": [input(&spent, json!(0))],
        });
        (request, vec![spent, outpoint("bb", 1), outpoint("dd", 5)])
    }

    fn proposal_listing(request: &Value, maker_inputs: Value) -> AtomicProposal {
        AtomicProposal {
            id: "01JATOMICSWAP".to_string(),
            status: "transaction.proposed".to_string(),
            proposal: json!({ "request": request, "maker_inputs": maker_inputs }),
            maker_inputs: vec![],
        }
    }

    #[test]
    fn a_proposal_lists_none_of_the_takers_coins_as_the_makers() {
        let (request, wallet) = taker_side();
        let maker = outpoint("cc", 2);

        // The maker's own coins come back, in order, for the record.
        let answer = proposal_listing(&request, json!([input(&maker, json!(2))]));
        let listed = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap();
        assert_eq!(listed, vec![maker.clone()]);

        // Any coin of the wallet, not just the request's own inputs, listed
        // as the maker's: rgb-lib before #18 would sign it away. `vout` may
        // be a number or, as rgb-lib may write it, a string.
        for (coin, vout) in [
            (outpoint("aa", 0), json!(0)),
            (outpoint("dd", 5), json!(5)),
            (outpoint("dd", 5), json!("5")),
        ] {
            let answer = proposal_listing(
                &request,
                json!([input(&maker, json!(2)), input(&coin, vout)]),
            );
            let error = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("taker's own coin {coin}")),
                "{error}"
            );
        }

        // The same coin under another spelling of its txid: upper or mixed
        // case hex names the same 32 bytes.
        for spelling in ["DD".repeat(32), format!("Dd{}", "dd".repeat(31))] {
            let respelled = AtomicOutpoint {
                txid: spelling,
                vout: 5,
            };
            let answer = proposal_listing(&request, json!([input(&respelled, json!(5))]));
            let error = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap_err();
            assert!(error.to_string().contains("taker's own coin"), "{error}");
        }
        // A wallet list in uppercase still matches the proposal's lowercase.
        let upper: Vec<AtomicOutpoint> = wallet
            .iter()
            .map(|o| AtomicOutpoint {
                txid: o.txid.to_uppercase(),
                vout: o.vout,
            })
            .collect();
        let answer = proposal_listing(&request, json!([input(&outpoint("dd", 5), json!(5))]));
        assert!(check_proposal("01JATOMICSWAP", &request, &answer, &upper).is_err());
        // Returned maker inputs are canonical, lowercase.
        let answer = proposal_listing(
            &request,
            json!([input(
                &AtomicOutpoint {
                    txid: "CC".repeat(32),
                    vout: 2
                },
                json!(2)
            )]),
        );
        let listed = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap();
        assert_eq!(listed, vec![maker.clone()]);
        // One maker coin under two spellings is still listed twice.
        let answer = proposal_listing(
            &request,
            json!([
                input(&maker, json!(2)),
                input(
                    &AtomicOutpoint {
                        txid: "CC".repeat(32),
                        vout: 2
                    },
                    json!(2)
                ),
            ]),
        );
        let error = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap_err();
        assert!(error.to_string().contains("twice"), "{error}");
        // Not hex, or not 32 bytes: refused.
        for txid in ["zz".repeat(32), "aa".repeat(31)] {
            let answer = proposal_listing(
                &request,
                json!([input(&AtomicOutpoint { txid, vout: 0 }, json!(0))]),
            );
            assert!(check_proposal("01JATOMICSWAP", &request, &answer, &wallet).is_err());
        }
        let bad_wallet = vec![
            wallet[0].clone(),
            AtomicOutpoint {
                txid: "nope".into(),
                vout: 0,
            },
        ];
        assert!(check_wallet_outpoints(&request, &bad_wallet).is_err());

        // The same maker input twice.
        let answer = proposal_listing(
            &request,
            json!([input(&maker, json!(2)), input(&maker, json!(2))]),
        );
        let error = check_proposal("01JATOMICSWAP", &request, &answer, &wallet).unwrap_err();
        assert!(error.to_string().contains("twice"), "{error}");

        // A list rgb-lib could not have written is refused, not skipped.
        for malformed in [json!({}), json!([{ "outpoint": { "txid": "cc" } }])] {
            let answer = proposal_listing(&request, malformed);
            assert!(check_proposal("01JATOMICSWAP", &request, &answer, &wallet).is_err());
        }
    }

    #[test]
    fn the_wallet_list_must_hold_the_requests_own_inputs() {
        let (request, wallet) = taker_side();
        check_wallet_outpoints(&request, &wallet).unwrap();
        // An empty or partial list, e.g. one passed by mistake, protects
        // nothing: refused before anything is sent.
        for partial in [vec![], wallet[1..].to_vec()] {
            let error = check_wallet_outpoints(&request, &partial).unwrap_err();
            assert!(error.to_string().contains("list_unspents"), "{error}");
        }
    }

    #[test]
    fn a_completion_must_broadcast_what_was_signed() {
        let proposal = json!({ "txid": "aa".repeat(32) });
        let completion = json!({ "proposal": proposal, "txid": "aa".repeat(32) });
        let answer = AtomicCompletion {
            id: "01JATOMICSWAP".to_string(),
            status: "transaction.mempool".to_string(),
            txid: "aa".repeat(32),
            completion: json!({ "proposal": proposal, "txid": "aa".repeat(32), "finalized_psbt": "x" }),
        };
        check_completion("01JATOMICSWAP", &completion, &answer).unwrap();

        let mut other_tx = answer.clone();
        other_tx.txid = "bb".repeat(32);
        assert!(check_completion("01JATOMICSWAP", &completion, &other_tx).is_err());

        let mut other_proposal = answer.clone();
        other_proposal.completion["proposal"] = json!({ "txid": "cc".repeat(32) });
        assert!(check_completion("01JATOMICSWAP", &completion, &other_proposal).is_err());
    }

    #[test]
    fn status_and_pairs_parse_the_maker_shapes() {
        let status: AtomicSwapStatus = serde_json::from_value(json!({
            "id": "01JATOMICSWAP", "pair": "BTC/USDT-RGB",
            "status": "transaction.confirmed", "state": "confirmed",
            "fromAmount": 100_000, "toAmount": 95_000_000, "assetId": ASSET,
            "networkFeeSat": 1_200, "txid": "aa".repeat(32), "confirmations": 1,
            "expiresAt": NOW + 60, "offerExpiresAt": NOW + 300, "createdAt": NOW
        }))
        .unwrap();
        assert_eq!(status.status, AtomicSwapState::Confirmed);
        assert!(!status.status.is_terminal());
        assert!(AtomicSwapState::Claimed.is_terminal() && AtomicSwapState::Claimed.is_settled());
        assert!(
            AtomicSwapState::Cancelled.is_terminal() && !AtomicSwapState::Cancelled.is_settled()
        );

        let pairs: GetAtomicPairsResponse = serde_json::from_value(json!({
            "BTC": { "USDT-RGB": {
                "pairId": "BTC/USDT-RGB", "hash": "ab", "rate": 95_000.0,
                "limits": { "minimal": 10_000, "maximal": 1_000_000 },
                "fees": { "percentage": 0.5, "minerFees": 0 }
            } }
        }))
        .unwrap();
        assert_eq!(
            pairs.get("BTC", "USDT-RGB").unwrap().limits.maximal,
            1_000_000
        );
        let halted: GetAtomicPairsResponse = serde_json::from_value(json!({})).unwrap();
        assert!(halted.get("BTC", "USDT-RGB").is_none());
    }

    /// Answer each request with the next canned `(status, body)` and record
    /// `METHOD path body` for each.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn mock_maker(answers: Vec<(u16, Value)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v2", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let head_end = loop {
                    let read = stream.read(&mut buf).unwrap();
                    raw.extend_from_slice(&buf[..read]);
                    if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let len: usize = String::from_utf8_lossy(&raw[..end])
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse().ok())?
                            })
                            .unwrap_or(0);
                        if raw.len() >= end + 4 + len {
                            break end;
                        }
                    }
                };
                let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
                let line = head.lines().next().unwrap_or_default();
                let mut parts = line.split(' ');
                let method = parts.next().unwrap_or_default();
                let path = parts.next().unwrap_or_default();
                let sent = String::from_utf8_lossy(&raw[head_end + 4..]).into_owned();
                seen.push(format!("{method} {path} {sent}").trim_end().to_string());
                let body = body.to_string();
                write!(
                    stream,
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            seen
        });
        (base_url, handle)
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn the_client_runs_the_atomic_flow() {
        use crate::swaps::boltz::BoltzApiClientV2;

        let q = quote();
        let offer = q.offer.clone();
        let wallet_request =
            json!({ "offer": offer, "taker_inputs": [], "taker_change_script_pubkey_hex": "0014" });
        let proposal =
            json!({ "request": wallet_request, "txid": "aa".repeat(32), "psbt": "cHNidP8=" });
        let signed = json!({ "proposal": proposal, "txid": "aa".repeat(32), "psbt": "cHNidP8=" });
        let (url, server) = mock_maker(vec![
            (201, serde_json::to_value(&q).unwrap()),
            (
                200,
                json!({ "id": q.id, "status": "transaction.proposed", "proposal": proposal }),
            ),
            (
                200,
                json!({ "id": q.id, "status": "transaction.mempool", "txid": "aa".repeat(32), "completion": signed }),
            ),
            (
                200,
                json!({
                    "id": q.id, "pair": q.pair, "status": "transaction.claimed", "state": "settled",
                    "fromAmount": q.from_amount, "toAmount": q.to_amount, "assetId": ASSET,
                    "networkFeeSat": q.network_fee_sat, "txid": "aa".repeat(32), "confirmations": 1,
                    "expiresAt": q.expires_at, "offerExpiresAt": q.offer_expires_at, "createdAt": NOW
                }),
            ),
        ]);
        let client = BoltzApiClientV2::new(url, None);

        let quoted = client
            .atomic_quote(&request(), &expect(), NOW)
            .await
            .unwrap();
        assert_eq!(quoted, q);
        let proposed = client
            .atomic_request(&q.id, &wallet_request, &[])
            .await
            .unwrap();
        assert_eq!(proposed.proposal, proposal);
        let done = client.atomic_complete(&q.id, &signed).await.unwrap();
        assert_eq!(done.txid, "aa".repeat(32));
        let status = client.atomic_swap(&q.id).await.unwrap();
        assert!(status.status.is_settled());

        let seen = server.join().unwrap();
        assert_eq!(
            seen[0],
            r#"POST /v2/swap/atomic/quote {"pair":"BTC/USDT-RGB","amount":100000,"direction":"from"}"#
        );
        assert!(seen[1].starts_with(&format!(
            "POST /v2/swap/atomic/{}/request {{\"request\":",
            q.id
        )));
        assert!(seen[2].starts_with(&format!(
            "POST /v2/swap/atomic/{}/complete {{\"completion\":",
            q.id
        )));
        assert_eq!(seen[3], format!("GET /v2/swap/atomic/{}", q.id));
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn the_client_refuses_a_quote_that_fails_verification() {
        use crate::swaps::boltz::BoltzApiClientV2;

        let mut q = quote();
        q.offer["maker_gives"]["amount"] = json!(1);
        let (url, server) = mock_maker(vec![(201, serde_json::to_value(&q).unwrap())]);
        let client = BoltzApiClientV2::new(url, None);
        let err = client
            .atomic_quote(&request(), &expect(), NOW)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("offer gives"), "{err}");
        server.join().unwrap();
    }
}
