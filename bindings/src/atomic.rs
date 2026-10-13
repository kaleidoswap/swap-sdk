//! Python surface for BTC L1 <-> USDT-RGB L1 atomic swaps — the maker's
//! `/v2/swap/atomic` wire. Mirrors `kaleidorg_swap_sdk::atomic`; see that
//! module for the flow and for what `verify` checks.
//!
//! rgb-lib's offer, request, proposal and completion cross as JSON text, as
//! the wallet's rgb-lib bindings take and return them.

use std::collections::HashMap;

use kaleidorg_swap_sdk::atomic::{
    self, AtomicAmountDirection, AtomicPair, AtomicPairFees, AtomicPairLimits,
    AtomicQuoteExpectations, AtomicQuoteRequest, AtomicSwapState, AtomicSwapStatus,
};

use crate::boltz::{Error, SwapClient};

#[uniffi::remote(Enum)]
pub enum AtomicAmountDirection {
    From,
    To,
}

#[uniffi::remote(Enum)]
pub enum AtomicSwapState {
    Created,
    Requested,
    Proposed,
    Mempool,
    Confirmed,
    Claimed,
    Expired,
    Cancelled,
    Failed,
}

#[uniffi::remote(Record)]
pub struct AtomicQuoteRequest {
    pub pair: String,
    pub amount: u64,
    pub direction: AtomicAmountDirection,
}

#[uniffi::remote(Record)]
pub struct AtomicQuoteExpectations {
    pub asset_id: String,
    pub bitcoin_network: String,
    pub proxy_url: Option<String>,
    pub max_network_fee_sat: Option<u64>,
    pub max_service_fee: Option<u64>,
    pub min_to_amount: Option<u64>,
    pub max_from_amount: Option<u64>,
    pub max_offer_ttl_secs: Option<u64>,
}

#[uniffi::remote(Record)]
pub struct AtomicSwapStatus {
    pub id: String,
    pub pair: String,
    pub status: AtomicSwapState,
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
    pub hash: String,
    pub rate: f64,
    pub limits: AtomicPairLimits,
    pub fees: AtomicPairFees,
}

/// A verified quote. `offer_json` is rgb-lib's `OnchainSwapOffer`, for the
/// wallet's `accept_swap_offer`.
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

impl From<atomic::AtomicQuote> for AtomicQuote {
    fn from(q: atomic::AtomicQuote) -> Self {
        Self {
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
            offer_json: q.offer.to_string(),
        }
    }
}

/// The maker's unsigned `OnchainSwapProposal`, for the wallet's
/// `complete_swap_proposal`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AtomicProposal {
    pub id: String,
    pub status: String,
    pub proposal_json: String,
}

/// The broadcast swap. `completion_json` is the finalized
/// `OnchainSwapCompletion`, for the wallet's `accept_swap_transfers`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AtomicCompletion {
    pub id: String,
    pub status: String,
    pub txid: String,
    pub completion_json: String,
}

fn message(json: &str, name: &str) -> Result<serde_json::Value, Error> {
    serde_json::from_str(json).map_err(|e| Error::Generic(format!("{name} is not JSON: {e}")))
}

#[uniffi::export(async_runtime = "tokio")]
impl SwapClient {
    /// `GET /v2/swap/atomic/pairs`: from currency -> to currency -> card.
    #[uniffi::method]
    pub async fn get_atomic_pairs(
        &self,
    ) -> Result<HashMap<String, HashMap<String, AtomicPair>>, Error> {
        Ok(self.inner.atomic_pairs().await?.pairs)
    }

    /// Quote an atomic swap and verify it against `request` and
    /// `expectations` at `now_secs` (unix seconds).
    #[uniffi::method]
    pub async fn atomic_quote(
        &self,
        request: AtomicQuoteRequest,
        expectations: AtomicQuoteExpectations,
        now_secs: u64,
    ) -> Result<AtomicQuote, Error> {
        Ok(self
            .inner
            .atomic_quote(&request, &expectations, now_secs)
            .await?
            .into())
    }

    /// Post the wallet's `OnchainSwapRequest` JSON; returns the maker's
    /// unsigned proposal.
    #[uniffi::method]
    pub async fn atomic_request(
        &self,
        swap_id: String,
        request_json: String,
    ) -> Result<AtomicProposal, Error> {
        let request = message(&request_json, "request_json")?;
        let answer = self.inner.atomic_request(&swap_id, &request).await?;
        Ok(AtomicProposal {
            id: answer.id,
            status: answer.status,
            proposal_json: answer.proposal.to_string(),
        })
    }

    /// Post the wallet's signed `OnchainSwapCompletion` JSON; the maker signs
    /// last and broadcasts.
    #[uniffi::method]
    pub async fn atomic_complete(
        &self,
        swap_id: String,
        completion_json: String,
    ) -> Result<AtomicCompletion, Error> {
        let completion = message(&completion_json, "completion_json")?;
        let answer = self.inner.atomic_complete(&swap_id, &completion).await?;
        Ok(AtomicCompletion {
            id: answer.id,
            status: answer.status,
            txid: answer.txid,
            completion_json: answer.completion.to_string(),
        })
    }

    /// `GET /v2/swap/atomic/{id}`. Poll until the status is terminal.
    #[uniffi::method]
    pub async fn get_atomic_swap(&self, swap_id: String) -> Result<AtomicSwapStatus, Error> {
        Ok(self.inner.atomic_swap(&swap_id).await?)
    }
}
