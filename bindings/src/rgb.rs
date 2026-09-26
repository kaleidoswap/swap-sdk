//! RGB (USDT-RGB on Bitcoin L1) for the native bindings. Mirrors
//! `kaleidorg_swap_sdk::rgb`; see that module for the checks and for how the
//! caller's rgb-lib wallet fits in. rgb-lib's swap messages cross as JSON
//! strings (`*_json`), which UniFFI can carry and rgb-lib's own bindings
//! read.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin::key::PublicKey;
use bitcoin::{Amount, OutPoint, Psbt, ScriptBuf, Transaction, TxOut};
use kaleidorg_swap_sdk::boltz::{
    CreateReverseRequest as CoreCreateReverseRequest, CreateReverseResponse,
    CreateSubmarineRequest as CoreCreateSubmarineRequest, CreateSubmarineResponse,
};
use kaleidorg_swap_sdk::network::BitcoinChain;
use kaleidorg_swap_sdk::rgb::{
    self as core_rgb, AtomicAmountDirection, AtomicPair, AtomicPairFees, AtomicPairLimits,
    AtomicQuoteRequest, AtomicSwapStatus, GetAtomicPairsResponse, RgbLock,
};
use kaleidorg_swap_sdk::swaps::BtcLikeTransaction as CoreBtcLikeTransaction;

use crate::boltz::{Error, SwapClient};
use crate::swap::{BtcLikeTransaction, KeyPair};
use crate::util::Preimage;

#[uniffi::remote(Enum)]
pub enum AtomicAmountDirection {
    From,
    To,
}

#[uniffi::remote(Record)]
pub struct AtomicQuoteRequest {
    pub pair: String,
    pub amount: u64,
    pub direction: AtomicAmountDirection,
}

#[uniffi::remote(Record)]
pub struct AtomicPairLimits {
    pub minimal: u64,
    pub maximal: u64,
}

#[uniffi::remote(Record)]
pub struct AtomicPairFees {
    pub percentage: f64,
    pub miner_fees: u64,
}

#[uniffi::remote(Record)]
pub struct AtomicPair {
    pub pair_id: Option<String>,
    pub hash: String,
    pub rate: f64,
    pub limits: AtomicPairLimits,
    pub fees: AtomicPairFees,
}

#[uniffi::remote(Record)]
pub struct GetAtomicPairsResponse {
    pub pairs: HashMap<String, HashMap<String, AtomicPair>>,
}

#[uniffi::remote(Record)]
pub struct AtomicSwapStatus {
    pub id: String,
    pub pair: String,
    pub status: String,
    pub state: String,
    pub from_amount: u64,
    pub to_amount: u64,
    pub asset_id: String,
    pub network_fee_sat: u64,
    pub txid: Option<String>,
    pub confirmations: u32,
    pub expires_at: i64,
    pub offer_expires_at: i64,
    pub created_at: i64,
}

/// An atomic quote, with rgb-lib's offer as the JSON string `offer_json`
/// for the taker's `accept_swap_offer`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AtomicQuote {
    pub id: String,
    pub pair: String,
    pub direction: AtomicAmountDirection,
    pub from_amount: u64,
    pub to_amount: u64,
    pub asset_id: String,
    pub network_fee_sat: u64,
    pub service_fee: u64,
    pub expires_at: i64,
    pub offer_expires_at: i64,
    pub offer_json: String,
}

impl AtomicQuote {
    fn from_core(q: core_rgb::AtomicQuoteResponse) -> Self {
        Self {
            offer_json: q.offer.to_string(),
            id: q.id,
            pair: q.pair,
            direction: q.direction,
            from_amount: q.from_amount,
            to_amount: q.to_amount,
            asset_id: q.asset_id,
            network_fee_sat: q.network_fee_sat,
            service_fee: q.service_fee,
            expires_at: q.expires_at,
            offer_expires_at: q.offer_expires_at,
        }
    }

    fn to_core(&self) -> Result<core_rgb::AtomicQuoteResponse, Error> {
        Ok(core_rgb::AtomicQuoteResponse {
            id: self.id.clone(),
            pair: self.pair.clone(),
            direction: self.direction,
            from_amount: self.from_amount,
            to_amount: self.to_amount,
            asset_id: self.asset_id.clone(),
            network_fee_sat: self.network_fee_sat,
            service_fee: self.service_fee,
            expires_at: self.expires_at,
            offer_expires_at: self.offer_expires_at,
            offer: json_arg(&self.offer_json, "offer_json")?,
        })
    }
}

/// The maker's unsigned proposal, for the taker's `complete_swap_proposal`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AtomicProposal {
    pub id: String,
    pub status: String,
    pub proposal_json: String,
}

/// The broadcast swap and the finalized completion, for the taker's
/// `accept_swap_transfers`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AtomicCompletion {
    pub id: String,
    pub status: String,
    pub txid: String,
    pub completion_json: String,
}

/// What the caller requires of an RGB lock beyond the swap tree. `None`
/// keeps the default: any `rgb:` contract, a 10 000 sat submarine cap.
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct RgbLockExpectations {
    #[uniffi(default = None)]
    pub asset_id: Option<String>,
    #[uniffi(default = None)]
    pub max_submarine_htlc_sat: Option<u64>,
}

impl From<RgbLockExpectations> for core_rgb::RgbLockExpectations {
    fn from(e: RgbLockExpectations) -> Self {
        let defaults = core_rgb::RgbLockExpectations::default();
        Self {
            asset_id: e.asset_id,
            max_submarine_htlc_sat: e
                .max_submarine_htlc_sat
                .unwrap_or(defaults.max_submarine_htlc_sat),
        }
    }
}

/// An RGB submarine swap (`USDT-RGB → BTC`): the taker locks USDT-RGB and
/// the maker pays `invoice`.
#[derive(Debug, uniffi::Record)]
pub struct CreateRgbSubmarineRequest {
    pub invoice: String,
    pub refund_public_key: PublicKey,
    #[uniffi(default = None)]
    pub pair_hash: Option<String>,
    #[uniffi(default = None)]
    pub referral_id: Option<String>,
}

/// An RGB reverse swap (`BTC → USDT-RGB`): the taker pays the maker's hold
/// invoice and claims the USDT-RGB the maker locks.
#[derive(Debug, uniffi::Record)]
pub struct CreateRgbReverseRequest {
    pub preimage_hash: String,
    pub claim_public_key: PublicKey,
    pub invoice_amount: u64,
    #[uniffi(default = None)]
    pub pair_hash: Option<String>,
    #[uniffi(default = None)]
    pub description: Option<String>,
    #[uniffi(default = None)]
    pub referral_id: Option<String>,
}

/// A P2TR key-path UTXO of the caller's wallet that pays a refund's fee.
#[derive(Debug, uniffi::Record)]
pub struct RgbFeeInput {
    /// `txid:vout`.
    pub outpoint: String,
    pub value_sat: u64,
    pub script_pubkey_hex: String,
    pub change_script_hex: String,
}

fn generic<E: std::fmt::Display>(context: &str) -> impl FnOnce(E) -> Error + '_ {
    move |e| Error::Generic(format!("{context}: {e}"))
}

fn json_arg(text: &str, param: &str) -> Result<serde_json::Value, Error> {
    serde_json::from_str(text).map_err(generic(&format!("{param} is not JSON")))
}

fn script_arg(hex: &str, param: &str) -> Result<ScriptBuf, Error> {
    ScriptBuf::from_hex(hex).map_err(generic(&format!("{param} is not a hex script")))
}

fn tx_arg(hex: &str, param: &str) -> Result<Transaction, Error> {
    bitcoin::consensus::encode::deserialize_hex(hex)
        .map_err(generic(&format!("{param} is not a hex transaction")))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[uniffi::export(async_runtime = "tokio")]
impl SwapClient {
    /// Create an RGB submarine swap, returned only once its swap tree and
    /// its `rgb` lock both validated.
    #[uniffi::method]
    pub async fn create_rgb_submarine_swap(
        &self,
        chain: BitcoinChain,
        swap_request: CreateRgbSubmarineRequest,
        expectations: RgbLockExpectations,
    ) -> Result<CreateSubmarineResponse, Error> {
        let req = CoreCreateSubmarineRequest {
            from: core_rgb::USDT_RGB.to_string(),
            to: "BTC".to_string(),
            invoice: swap_request.invoice,
            refund_public_key: swap_request.refund_public_key,
            pair_hash: swap_request.pair_hash,
            referral_id: swap_request.referral_id,
            webhook: None,
        };
        Ok(self
            .inner
            .create_rgb_submarine_swap(&req, chain, &expectations.into())
            .await?)
    }

    /// Create an RGB reverse swap, returned only once its swap tree and its
    /// `rgb` lock both validated.
    #[uniffi::method]
    pub async fn create_rgb_reverse_swap(
        &self,
        chain: BitcoinChain,
        swap_request: CreateRgbReverseRequest,
        expectations: RgbLockExpectations,
    ) -> Result<CreateReverseResponse, Error> {
        let req = CoreCreateReverseRequest {
            from: "BTC".to_string(),
            to: core_rgb::USDT_RGB.to_string(),
            claim_public_key: swap_request.claim_public_key,
            invoice: None,
            invoice_amount: Some(swap_request.invoice_amount),
            preimage_hash: Some(
                swap_request
                    .preimage_hash
                    .parse()
                    .map_err(generic("preimage_hash"))?,
            ),
            description: swap_request.description,
            description_hash: None,
            address: None,
            address_signature: None,
            referral_id: swap_request.referral_id,
            webhook: None,
            pair_hash: swap_request.pair_hash,
        };
        Ok(self
            .inner
            .create_rgb_reverse_swap(req, chain, &expectations.into())
            .await?)
    }

    /// `GET /v2/swap/atomic/pairs`.
    #[uniffi::method]
    pub async fn get_atomic_pairs(&self) -> Result<GetAtomicPairsResponse, Error> {
        Ok(self.inner.get_atomic_pairs().await?)
    }

    /// `POST /v2/swap/atomic/quote`, validated against the request and, when
    /// given, the expected contract.
    #[uniffi::method]
    pub async fn atomic_quote(
        &self,
        request: AtomicQuoteRequest,
        expected_asset_id: Option<String>,
    ) -> Result<AtomicQuote, Error> {
        let quote = self.inner.post_atomic_quote(&request).await?;
        quote.validate(&request, expected_asset_id.as_deref(), now_unix())?;
        Ok(AtomicQuote::from_core(quote))
    }

    /// `POST /v2/swap/atomic/{id}/request` with rgb-lib's
    /// `OnchainSwapRequest`, the proposal checked to belong to `quote`.
    #[uniffi::method]
    pub async fn atomic_request(
        &self,
        quote: AtomicQuote,
        request_json: String,
    ) -> Result<AtomicProposal, Error> {
        let quote = quote.to_core()?;
        let request = json_arg(&request_json, "request_json")?;
        let proposal = self.inner.post_atomic_request(&quote.id, request).await?;
        proposal.validate(&quote)?;
        Ok(AtomicProposal {
            proposal_json: proposal.proposal.to_string(),
            id: proposal.id,
            status: proposal.status,
        })
    }

    /// `POST /v2/swap/atomic/{id}/complete` with the taker-signed
    /// `OnchainSwapCompletion`; the maker signs last and broadcasts.
    #[uniffi::method]
    pub async fn atomic_complete(
        &self,
        quote: AtomicQuote,
        completion_json: String,
    ) -> Result<AtomicCompletion, Error> {
        let quote = quote.to_core()?;
        let completion = json_arg(&completion_json, "completion_json")?;
        let done = self
            .inner
            .post_atomic_complete(&quote.id, completion)
            .await?;
        done.validate(&quote)?;
        Ok(AtomicCompletion {
            completion_json: done.completion.to_string(),
            id: done.id,
            status: done.status,
            txid: done.txid,
        })
    }

    /// `GET /v2/swap/atomic/{id}`.
    #[uniffi::method]
    pub async fn get_atomic_swap(&self, swap_id: &str) -> Result<AtomicSwapStatus, Error> {
        Ok(self.inner.get_atomic_swap(swap_id).await?)
    }
}

/// Check the script rgb-lib decodes from `lock.recipient_id` against the
/// HTLC. Call it before a submarine lock.
#[uniffi::export]
pub fn rgb_check_recipient_script(lock: RgbLock, script_hex: String) -> Result<(), Error> {
    Ok(lock.check_recipient_script(&script_arg(&script_hex, "script_hex")?)?)
}

/// A colored claim or refund of an RGB HTLC.
#[derive(Debug, uniffi::Object)]
pub struct RgbHtlcSpend(core_rgb::RgbHtlcSpend);

#[uniffi::export]
impl RgbHtlcSpend {
    /// The taker's claim of a reverse lock.
    #[uniffi::constructor]
    pub fn claim(
        chain: BitcoinChain,
        response: CreateReverseResponse,
        our_pubkey: PublicKey,
        lock_tx_hex: String,
        dest_script_hex: String,
        fee_rate_sat_vb: Option<u64>,
    ) -> Result<Self, Error> {
        Ok(Self(core_rgb::RgbHtlcSpend::claim_from_response(
            &response,
            &our_pubkey,
            chain,
            &tx_arg(&lock_tx_hex, "lock_tx_hex")?,
            script_arg(&dest_script_hex, "dest_script_hex")?,
            fee_rate_sat_vb,
        )?))
    }

    /// The taker's refund of its own submarine lock after the timeout.
    #[uniffi::constructor]
    pub fn refund(
        chain: BitcoinChain,
        response: CreateSubmarineResponse,
        our_pubkey: PublicKey,
        lock_tx_hex: String,
        dest_script_hex: String,
        fee_input: Option<RgbFeeInput>,
        fee_rate_sat_vb: u64,
    ) -> Result<Self, Error> {
        let fee_input = fee_input
            .map(|f| -> Result<_, Error> {
                Ok(core_rgb::RgbFeeInput {
                    outpoint: OutPoint::from_str(&f.outpoint)
                        .map_err(generic("fee_input.outpoint"))?,
                    txout: TxOut {
                        value: Amount::from_sat(f.value_sat),
                        script_pubkey: script_arg(
                            &f.script_pubkey_hex,
                            "fee_input.script_pubkey_hex",
                        )?,
                    },
                    change_script: script_arg(&f.change_script_hex, "fee_input.change_script_hex")?,
                })
            })
            .transpose()?;
        Ok(Self(core_rgb::RgbHtlcSpend::refund_from_response(
            &response,
            &our_pubkey,
            chain,
            &tx_arg(&lock_tx_hex, "lock_tx_hex")?,
            script_arg(&dest_script_hex, "dest_script_hex")?,
            fee_input,
            fee_rate_sat_vb,
        )?))
    }

    /// The unsigned PSBT (base64) for rgb-lib to color; output 1 receives
    /// the asset.
    #[uniffi::method]
    pub fn psbt(&self) -> Result<String, Error> {
        Ok(self.0.psbt()?.to_string())
    }

    /// The miner fee the spend pays, sats.
    #[uniffi::method]
    pub fn fee_sat(&self) -> u64 {
        self.0.fee_sat()
    }

    /// Sign the HTLC input of the PSBT rgb-lib colored (base64), returning it
    /// with that input finalized.
    #[uniffi::method]
    pub fn sign_colored(
        &self,
        colored_psbt: String,
        keys: &KeyPair,
        preimage: Option<Arc<Preimage>>,
    ) -> Result<String, Error> {
        let psbt = Psbt::from_str(&colored_psbt).map_err(generic("colored_psbt"))?;
        Ok(self
            .0
            .sign_colored(&psbt, &keys.inner, preimage.as_ref().map(|p| &p.0))?
            .to_string())
    }

    /// `sign_colored` for a spend whose only input is the HTLC: the
    /// broadcastable transaction.
    #[uniffi::method]
    pub fn sign_colored_tx(
        &self,
        colored_psbt: String,
        keys: &KeyPair,
        preimage: Option<Arc<Preimage>>,
    ) -> Result<BtcLikeTransaction, Error> {
        let psbt = Psbt::from_str(&colored_psbt).map_err(generic("colored_psbt"))?;
        let tx = self
            .0
            .sign_colored_tx(&psbt, &keys.inner, preimage.as_ref().map(|p| &p.0))?;
        Ok(BtcLikeTransaction(CoreBtcLikeTransaction::bitcoin(tx)))
    }
}
