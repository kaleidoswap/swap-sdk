use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::Keypair;
use bitcoin::{consensus, Amount, Transaction as BtcTransaction};
use elements::{confidential, Transaction as LbtcTransaction};
use secp256k1_musig::musig;
use serde_json::Value;

use super::boltz::{
    BoltzApiClientV2, ChainSwapDetails, Cooperative, CreateReverseResponse,
    CreateSubmarineResponse, Side, SwapTxKind, SwapType,
};
use crate::boltz::TransactionInfo;
use crate::error::Error;
use crate::network::{
    BitcoinClient, Chain, LiquidChain, LiquidClient, Network, OutputSpend, OutputStatus,
};
use crate::swaps::bitcoin::{BtcSwapScript, BtcSwapTx};
use crate::swaps::fees::estimate_claim_fee;
use crate::swaps::liquid::{
    decode_swap_output, LiquidSwapScript, LiquidSwapTx, PreparedLiquidSpend,
};
use crate::swaps::rgb::{PreparedRgbSpend, RgbSpendFunding};
use crate::util::fees::Fee;
use crate::util::invoice::LightningInvoice;
use crate::util::secrets::Preimage;

#[derive(Clone)]
struct ChainClaim {
    refund_keys: Keypair,
    lockup_script: SwapScript,
}

#[derive(Clone, Default)]
pub struct DirectTxOptions {
    blinding_key: Option<bitcoin::secp256k1::SecretKey>,
}

impl fmt::Debug for DirectTxOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirectTxOptions")
            .field("has_blinding_key", &self.blinding_key.is_some())
            .finish()
    }
}

impl DirectTxOptions {
    pub fn new() -> Self {
        Self { blinding_key: None }
    }

    pub fn with_blinding_key(mut self, blinding_key: bitcoin::secp256k1::SecretKey) -> Self {
        self.blinding_key = Some(blinding_key);
        self
    }
}

/// What a claim must see on chain before the preimage leaves this process.
///
/// A claim reveals the preimage — cooperatively to the maker, or on chain
/// once broadcast — and the preimage is the only thing keeping the maker
/// from settling the other leg of the swap. So a claim is only safe against
/// a lockup that is actually in the chain, is not spent or being replaced,
/// and whose refund timeout is still comfortably ahead of the tip. The
/// maker's status strings are not evidence of any of this; the chain client
/// is. See `SwapScript::construct_claim`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimPolicy {
    /// Blocks the lockup must be buried under before it counts, tip block
    /// included. Must be at least 1 unless `allow_zero_conf` is set.
    pub min_confirmations: u32,
    /// Accept a lockup the chain source has only in its mempool, or has not
    /// seen at all. This is an explicit acceptance of the replaceable-lockup
    /// risk and is never the default.
    pub allow_zero_conf: bool,
    /// Refuse to claim once the tip is within this many blocks of the swap's
    /// refund timeout. `None` picks the chain default: 6 on Bitcoin, 60 on
    /// Liquid, both roughly an hour.
    pub claim_safety_margin_blocks: Option<u32>,
    /// Build the claim although an *unconfirmed* spend of the lockup is
    /// already visible. That spend is normally a conflict and refused; the
    /// one legitimate case is fee-bumping a stuck claim of your own, where
    /// the preimage is already public. A confirmed spend is always refused.
    pub allow_conflicting_spend: bool,
}

impl Default for ClaimPolicy {
    fn default() -> Self {
        Self {
            min_confirmations: 1,
            allow_zero_conf: false,
            claim_safety_margin_blocks: None,
            allow_conflicting_spend: false,
        }
    }
}

impl ClaimPolicy {
    pub const BITCOIN_SAFETY_MARGIN_BLOCKS: u32 = 6;
    pub const LIQUID_SAFETY_MARGIN_BLOCKS: u32 = 60;

    fn safety_margin_blocks(&self, chain: Chain) -> u32 {
        self.claim_safety_margin_blocks.unwrap_or(match chain {
            Chain::Bitcoin(_) => Self::BITCOIN_SAFETY_MARGIN_BLOCKS,
            Chain::Liquid(_) => Self::LIQUID_SAFETY_MARGIN_BLOCKS,
        })
    }

    /// Decide whether a claim of `outpoint` may go ahead, from what a trusted
    /// chain source reports (`tip_height`, `status`) and the script's refund
    /// timeout. Pure, so every refusal here is unit-tested without a chain.
    pub(crate) fn check(
        &self,
        chain: Chain,
        tip_height: u32,
        status: Option<OutputStatus>,
        timeout_block_height: Option<u32>,
        outpoint: &str,
    ) -> Result<(), Error> {
        if self.min_confirmations == 0 && !self.allow_zero_conf {
            return Err(Error::Protocol(
                "min_confirmations must be at least 1 unless allow_zero_conf is set".to_string(),
            ));
        }
        let Some(timeout_block_height) = timeout_block_height else {
            return Err(Error::Protocol(
                "Swap timeout is not a block height; refusing to claim without a deadline check"
                    .to_string(),
            ));
        };
        let margin = self.safety_margin_blocks(chain);
        if tip_height.saturating_add(margin) >= timeout_block_height {
            return Err(Error::Protocol(format!(
                "Too close to the swap timeout to claim safely: tip {tip_height} + margin {margin} blocks reaches timeout {timeout_block_height}; the counterparty's refund is or is about to become valid"
            )));
        }
        match status {
            None if self.allow_zero_conf => {
                log::warn!("Claiming {outpoint} before the chain source has seen its lockup (allow_zero_conf)");
            }
            None => {
                return Err(Error::Protocol(format!(
                    "Lockup {outpoint} is unknown to the chain source; refusing to reveal the preimage against it"
                )));
            }
            Some(status) => {
                match status.spend {
                    OutputSpend::Unspent => {}
                    OutputSpend::SpentConfirmed => {
                        return Err(Error::Protocol(format!(
                            "Lockup {outpoint} is already spent"
                        )));
                    }
                    OutputSpend::SpentUnconfirmed if self.allow_conflicting_spend => {
                        log::warn!("Claiming {outpoint} over an unconfirmed spend (allow_conflicting_spend)");
                    }
                    OutputSpend::SpentUnconfirmed => {
                        return Err(Error::Protocol(format!(
                            "Lockup {outpoint} has a conflicting spend in the mempool; refusing to reveal the preimage against it"
                        )));
                    }
                }
                let confirmations = status.confirmations(tip_height);
                if !self.allow_zero_conf && confirmations < self.min_confirmations {
                    return Err(Error::Protocol(format!(
                        "Lockup {outpoint} has {confirmations} confirmations, {} required before the preimage is revealed",
                        self.min_confirmations
                    )));
                }
                if confirmations == 0 {
                    log::warn!("Claiming unconfirmed lockup {outpoint} (allow_zero_conf)");
                }
            }
        }
        Ok(())
    }

    async fn ensure_bitcoin_claimable(
        &self,
        client: &dyn BitcoinClient,
        outpoint: &bitcoin::OutPoint,
        script_pubkey: &bitcoin::Script,
        locktime: bitcoin::absolute::LockTime,
    ) -> Result<(), Error> {
        let tip = client.get_tip_height().await?;
        let status = client.get_output_status(outpoint, script_pubkey).await?;
        let timeout = match locktime {
            bitcoin::absolute::LockTime::Blocks(height) => Some(height.to_consensus_u32()),
            bitcoin::absolute::LockTime::Seconds(_) => None,
        };
        self.check(
            Chain::Bitcoin(client.network()),
            tip,
            status,
            timeout,
            &outpoint.to_string(),
        )
    }

    async fn ensure_liquid_claimable(
        &self,
        client: &dyn LiquidClient,
        outpoint: &elements::OutPoint,
        script_pubkey: &elements::Script,
        locktime: elements::LockTime,
    ) -> Result<(), Error> {
        let tip = client.get_tip_height().await?;
        let status = client.get_output_status(outpoint, script_pubkey).await?;
        let timeout = match locktime {
            elements::LockTime::Blocks(height) => Some(height.to_consensus_u32()),
            elements::LockTime::Seconds(_) => None,
        };
        self.check(
            Chain::Liquid(client.network()),
            tip,
            status,
            timeout,
            &outpoint.to_string(),
        )
    }
}

#[derive(Clone)]
pub struct TransactionOptions {
    cooperative: bool,
    swap_auth: Option<String>,
    chain_claim: Option<ChainClaim>,
    lockup_tx: Option<BtcLikeTransaction>,
    broadcast_lockup_tx: bool,
    additional_outputs: Vec<(String, u64)>,
    claim_policy: ClaimPolicy,
}

impl fmt::Debug for TransactionOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionOptions")
            .field("cooperative", &self.cooperative)
            .field("has_chain_claim", &self.chain_claim.is_some())
            .field("lockup_tx", &self.lockup_tx)
            .field("claim_policy", &self.claim_policy)
            .finish()
    }
}

impl Default for TransactionOptions {
    fn default() -> Self {
        Self {
            cooperative: true,
            swap_auth: None,
            chain_claim: None,
            lockup_tx: None,
            broadcast_lockup_tx: true,
            additional_outputs: Vec::new(),
            claim_policy: ClaimPolicy::default(),
        }
    }
}

impl TransactionOptions {
    /// Whether a cooperative claim with boltz should be attempted
    pub fn with_cooperative(mut self, cooperative: bool) -> Self {
        self.cooperative = cooperative;
        self
    }

    /// The chain evidence a claim needs before revealing the preimage; see
    /// [`ClaimPolicy`]. Refunds ignore it.
    pub fn with_claim_policy(mut self, policy: ClaimPolicy) -> Self {
        self.claim_policy = policy;
        self
    }

    /// Confirmations the lockup needs before a claim reveals the preimage
    /// (default 1).
    pub fn with_min_confirmations(mut self, min_confirmations: u32) -> Self {
        self.claim_policy.min_confirmations = min_confirmations;
        self
    }

    /// Explicitly accept claiming an unconfirmed, replaceable lockup.
    pub fn with_allow_zero_conf(mut self, allow_zero_conf: bool) -> Self {
        self.claim_policy.allow_zero_conf = allow_zero_conf;
        self
    }

    /// Blocks of headroom to the swap timeout a claim requires (default 6 on
    /// Bitcoin, 60 on Liquid). Zero accepts the expiry race.
    pub fn with_claim_safety_margin_blocks(mut self, blocks: u32) -> Self {
        self.claim_policy.claim_safety_margin_blocks = Some(blocks);
        self
    }

    /// Build a claim over an unconfirmed spend of the lockup, to fee-bump a
    /// stuck claim of your own. See [`ClaimPolicy::allow_conflicting_spend`].
    pub fn with_allow_conflicting_spend(mut self, allow: bool) -> Self {
        self.claim_policy.allow_conflicting_spend = allow;
        self
    }

    pub fn claim_policy(&self) -> &ClaimPolicy {
        &self.claim_policy
    }

    /// Credential returned on create or recovered with proof of swap-key ownership.
    pub fn with_swap_auth(mut self, swap_auth: String) -> Self {
        self.swap_auth = Some(swap_auth);
        self
    }

    /// For a cooperative claim of a chain swap, the refund keys and lockup script of the swap have to be provided
    /// Calling this function will implicitly set cooperative to true
    pub fn with_chain_claim(mut self, refund_keys: Keypair, lockup_script: SwapScript) -> Self {
        self.cooperative = true;
        self.chain_claim = Some(ChainClaim {
            refund_keys,
            lockup_script,
        });
        self
    }

    pub fn with_lockup_tx(mut self, lockup_tx: BtcLikeTransaction) -> Self {
        self.lockup_tx = Some(lockup_tx);
        self.broadcast_lockup_tx = true;
        self
    }

    /// Pin claim input discovery to an already observed lockup, without
    /// rebroadcasting it. Its chain inclusion is still checked against the
    /// chain client per the [`ClaimPolicy`] before a claim signs; ordinary
    /// script/address/amount validation runs as well.
    pub fn with_existing_lockup_tx(mut self, lockup_tx: BtcLikeTransaction) -> Self {
        self.lockup_tx = Some(lockup_tx);
        self.broadcast_lockup_tx = false;
        self
    }

    /// Extra fixed-amount outputs (in satoshis) paid in addition to the primary
    /// output address. The primary output receives the remainder
    /// (input - fee - sum of these) and remains the first payment output, with
    /// the additional outputs following in the given order. On Bitcoin the
    /// primary stays at output index 0; on Liquid claims order outputs
    /// [primary, additions.., fee] and refunds [fee, primary, additions..].
    ///
    /// Calling this again replaces the previously set list. Addresses must be
    /// valid for the chain client's network. On Liquid all amounts must be
    /// positive and the addresses must match the spend: confidential for a
    /// blinded spend, explicit for an explicit one. On both chains an explicit
    /// output below the dust threshold for its script type is rejected at
    /// construction.
    /// Cooperative signing commits to every output, including the additional
    /// ones.
    pub fn with_additional_outputs(mut self, additional_outputs: Vec<(String, u64)>) -> Self {
        self.additional_outputs = additional_outputs;
        self
    }
}

/// A wrapper for transactions that can be either Bitcoin or Liquid
#[derive(Clone, Debug)]
pub enum BtcLikeTransaction {
    Bitcoin(BtcTransaction),
    Liquid(LbtcTransaction),
}

impl BtcLikeTransaction {
    pub fn from_hex(chain: Chain, hex: &str) -> Result<Self, Error> {
        match chain {
            Chain::Bitcoin(_) => Self::from_hex_bitcoin(hex),
            Chain::Liquid(_) => Self::from_hex_liquid(hex),
        }
    }

    pub fn from_hex_bitcoin(hex: &str) -> Result<Self, Error> {
        let decoded = hex::decode(hex)?;
        Ok(Self::bitcoin(consensus::deserialize(&decoded)?))
    }

    pub fn from_hex_liquid(hex: &str) -> Result<Self, Error> {
        let decoded = hex::decode(hex)?;
        Ok(Self::liquid(elements::encode::deserialize(&decoded)?))
    }

    pub fn bitcoin(tx: BtcTransaction) -> Self {
        Self::Bitcoin(tx)
    }

    pub fn liquid(tx: LbtcTransaction) -> Self {
        Self::Liquid(tx)
    }

    pub fn as_bitcoin(&self) -> Option<&BtcTransaction> {
        match self {
            Self::Bitcoin(tx) => Some(tx),
            Self::Liquid(_) => None,
        }
    }

    pub fn as_liquid(&self) -> Option<&LbtcTransaction> {
        match self {
            Self::Bitcoin(_) => None,
            Self::Liquid(tx) => Some(tx),
        }
    }
}

/// A wrapper for blockchain clients that can be either Bitcoin or Liquid
pub struct ChainClient {
    bitcoin: Option<Box<dyn BitcoinClient>>,
    liquid: Option<Box<dyn LiquidClient>>,
}

impl Default for ChainClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainClient {
    pub fn new() -> Self {
        Self {
            bitcoin: None,
            liquid: None,
        }
    }

    pub fn with_bitcoin(mut self, client: impl BitcoinClient + 'static) -> Self {
        self.bitcoin = Some(Box::new(client));
        self
    }

    pub fn with_liquid(mut self, client: impl LiquidClient + 'static) -> Self {
        self.liquid = Some(Box::new(client));
        self
    }

    pub fn bitcoin_client(&self) -> Option<&dyn BitcoinClient> {
        self.bitcoin.as_deref()
    }

    pub fn liquid_client(&self) -> Option<&dyn LiquidClient> {
        self.liquid.as_deref()
    }

    fn require_bitcoin_client(&self) -> Result<&dyn BitcoinClient, Error> {
        self.bitcoin_client()
            .ok_or_else(|| Error::Generic("Expected Bitcoin client".to_string()))
    }

    fn require_liquid_client(&self) -> Result<&dyn LiquidClient, Error> {
        self.liquid_client()
            .ok_or_else(|| Error::Generic("Expected Liquid client".to_string()))
    }

    pub async fn broadcast_tx(&self, tx: &BtcLikeTransaction) -> Result<String, Error> {
        match tx {
            BtcLikeTransaction::Bitcoin(tx) => {
                let id = self.require_bitcoin_client()?.broadcast_tx(tx).await?;
                Ok(id.to_string())
            }
            BtcLikeTransaction::Liquid(tx) => {
                let id = self.require_liquid_client()?.broadcast_tx(tx).await?;
                Ok(id)
            }
        }
    }

    pub async fn try_broadcast_tx(&self, tx: &BtcLikeTransaction) -> Result<(), Error> {
        match self.broadcast_tx(tx).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.message().contains("already in block chain")
                    || e.message().contains("already in utxo set")
                {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Trait for common functionality between Bitcoin and Liquid swap transactions
pub trait SwapScriptCommon {
    fn swap_type(&self) -> SwapType;

    fn receiver_pubkey(&self) -> bitcoin::PublicKey;

    fn partial_sign(
        &self,
        keys: &Keypair,
        pub_nonce: &str,
        transaction_hash: &str,
    ) -> Result<(musig::PartialSignature, musig::PublicNonce), Error>;
}

/// A wrapper for swap scripts that can be either Bitcoin or Liquid
#[derive(Clone, Debug)]
pub enum SwapScriptImpl {
    Bitcoin(Arc<BtcSwapScript>),
    Liquid(Arc<LiquidSwapScript>),
}

#[derive(Clone, Debug)]
pub struct SwapScript {
    pub(super) script: SwapScriptImpl,
    pub(super) boltz_lockup: Option<Amount>,
    pub(super) mrh_amount: Option<Amount>,
}

#[derive(Clone)]
pub struct SwapTransactionParams<'a> {
    pub keys: Keypair,
    pub output_address: String,
    pub fee: Fee,
    pub swap_id: String,
    pub chain_client: &'a ChainClient,
    pub boltz_api: &'a BoltzApiClientV2,
    pub options: Option<TransactionOptions>,
}

#[derive(Clone)]
pub struct LiquidPsetParams<'a> {
    pub output_address: String,
    /// User/application fee ceiling.
    pub max_fee: u64,
    /// Fee ceiling from the accepted pair quote. The effective cap is the lower
    /// of this and `max_fee` and is pinned into the prepared spend.
    pub quoted_fee_cap: u64,
    pub swap_id: String,
    pub chain_client: &'a ChainClient,
    pub boltz_api: &'a BoltzApiClientV2,
    pub options: Option<TransactionOptions>,
}

/// Inputs to [`SwapScript::prepare_rgb_claim`] and
/// [`SwapScript::prepare_rgb_refund`].
#[derive(Clone)]
pub struct RgbPsbtParams<'a> {
    /// The colored payout: the address of the script an rgb-lib
    /// `witness_receive` returned (`script_buf_from_recipient_id`).
    pub output_address: String,
    pub funding: RgbSpendFunding,
    /// The highest fee the SDK will sign, in sats.
    pub max_fee: u64,
    pub swap_id: String,
    pub chain_client: &'a ChainClient,
    pub boltz_api: &'a BoltzApiClientV2,
    /// Required colored lock transaction: the taker's own lock for a refund
    /// (rgb-lib's `send` returns it), or the maker's from `get_reverse_tx` for
    /// a claim. Address discovery cannot distinguish a third-party BTC output
    /// from the output carrying the accepted RGB allocation.
    pub lockup_tx: BtcTransaction,
}

impl SwapScriptImpl {
    pub fn bitcoin(script: BtcSwapScript) -> Self {
        Self::Bitcoin(Arc::new(script))
    }

    pub fn liquid(script: LiquidSwapScript) -> Self {
        Self::Liquid(Arc::new(script))
    }

    pub fn common(&self) -> &dyn SwapScriptCommon {
        match self {
            Self::Bitcoin(script) => script.as_ref(),
            Self::Liquid(script) => script.as_ref(),
        }
    }
}

impl SwapScript {
    pub(super) fn new(
        script: SwapScriptImpl,
        boltz_lockup: Option<Amount>,
        mrh_amount: Option<Amount>,
    ) -> Self {
        Self {
            script,
            boltz_lockup,
            mrh_amount,
        }
    }

    /// Build from an already-reconstructed Bitcoin swap script (a swap
    /// restored from storage rather than freshly created), so the validated
    /// helpers — notably `submarine_cooperative_claim`, which verifies the
    /// preimage against the invoice before partial-signing — are reachable
    /// without the original swap response.
    pub fn from_bitcoin(script: BtcSwapScript) -> Self {
        Self::new(SwapScriptImpl::bitcoin(script), None, None)
    }

    /// Liquid counterpart of [`SwapScript::from_bitcoin`].
    pub fn from_liquid(script: LiquidSwapScript) -> Self {
        Self::new(SwapScriptImpl::liquid(script), None, None)
    }

    pub fn submarine_from_swap_resp(
        chain: Chain,
        create_swap_response: &CreateSubmarineResponse,
        our_pubkey: bitcoin::PublicKey,
    ) -> Result<Self, Error> {
        let script: Result<SwapScriptImpl, Error> = match chain {
            Chain::Bitcoin(_) => {
                let script =
                    BtcSwapScript::submarine_from_swap_resp(create_swap_response, our_pubkey)?;
                Ok(SwapScriptImpl::bitcoin(script))
            }
            Chain::Liquid(_) => {
                let script =
                    LiquidSwapScript::submarine_from_swap_resp(create_swap_response, our_pubkey)?;
                Ok(SwapScriptImpl::liquid(script))
            }
        };
        // we dont have to validate our own lockup amounts
        Ok(Self::new(script?, None, None))
    }

    pub fn reverse_from_swap_resp(
        chain: Chain,
        reverse_response: &CreateReverseResponse,
        our_pubkey: bitcoin::PublicKey,
    ) -> Result<Self, Error> {
        let script: Result<SwapScriptImpl, Error> = match chain {
            Chain::Bitcoin(_) => {
                let script = BtcSwapScript::reverse_from_swap_resp(reverse_response, our_pubkey)?;
                Ok(SwapScriptImpl::bitcoin(script))
            }
            Chain::Liquid(_) => {
                let script =
                    LiquidSwapScript::reverse_from_swap_resp(reverse_response, our_pubkey)?;
                Ok(SwapScriptImpl::liquid(script))
            }
        };

        let script = script?;
        // An RGB HTLC's on-chain value is its `htlcSat`; `onchainAmount` is
        // the asset amount, in the contract's units.
        let boltz_lockup = Amount::from_sat(match &script {
            SwapScriptImpl::Bitcoin(script) => script.expected_amount,
            SwapScriptImpl::Liquid(_) => reverse_response.onchain_amount,
        });
        let mrh_amount = match (&script, chain) {
            (SwapScriptImpl::Liquid(script), _) if !script.requires_caller_funded_pset() => {
                Some(boltz_lockup - estimate_claim_fee(chain, 0.1))
            }
            _ => None,
        };
        Ok(Self::new(script, Some(boltz_lockup), mrh_amount))
    }

    pub fn chain_from_swap_resp(
        chain: Chain,
        side: Side,
        chain_swap_details: ChainSwapDetails,
        our_pubkey: bitcoin::PublicKey,
    ) -> Result<Self, Error> {
        let amount = chain_swap_details.amount;
        let script: Result<SwapScriptImpl, Error> = match chain {
            Chain::Bitcoin(_) => {
                let script =
                    BtcSwapScript::chain_from_swap_resp(side, chain_swap_details, our_pubkey)?;
                Ok(SwapScriptImpl::bitcoin(script))
            }
            Chain::Liquid(_) => {
                let script =
                    LiquidSwapScript::chain_from_swap_resp(side, chain_swap_details, our_pubkey)?;
                Ok(SwapScriptImpl::liquid(script))
            }
        };
        Ok(Self::new(
            script?,
            if amount > 0 {
                Some(Amount::from_sat(amount))
            } else {
                None
            },
            None,
        ))
    }

    /// Cooperatively claim a submarine swap with Boltz.
    ///
    /// This function should be called when the swap status is `transaction.claim.pending`, indicating
    /// that Boltz has detected the on-chain funding transaction and has paid the invoice.
    /// The function will verify that boltz indeed has paid the given `invoice` by checking the returned preimage
    /// before sending the partial signature for the claim transaction of boltz.
    pub async fn submarine_cooperative_claim(
        &self,
        swap_id: &String,
        keys: &Keypair,
        invoice: &str,
        boltz_api: &BoltzApiClientV2,
    ) -> Result<Value, Error> {
        if self.script.common().swap_type() != SwapType::Submarine {
            return Err(Error::Generic(
                "can only be called for submarine swaps".to_string(),
            ));
        }
        // Get claim tx details from Boltz
        let claim_tx_response = boltz_api.get_submarine_claim_tx_details(swap_id).await?;

        // Verify preimage matches invoice payment hash
        verify_submarine_preimage(invoice, &claim_tx_response.preimage)?;

        self.validate_cooperative_counterparty(&claim_tx_response.public_key)?;

        // Generate partial signature
        let (partial_sig, pub_nonce) = self.script.common().partial_sign(
            keys,
            &claim_tx_response.pub_nonce.to_string(),
            &claim_tx_response.transaction_hash.to_string(),
        )?;

        boltz_api
            .post_submarine_claim_tx_details(swap_id, pub_nonce, partial_sig)
            .await
    }

    // Initiates a cooperative claim for a chain swap with Boltz.
    //
    // This function should be called when the swap status is `transaction.server.confirmed`,
    // It creates a partial signature for boltz's side of the transaction, and returns a Cooperative struct which
    // can be passed to `sign_claim` where it is used in exchange for the signature for our own claim transaction.
    pub async fn cooperative_chain_claim<'a>(
        &self,
        our_refund_keys: &Keypair,
        swap_id: &String,
        boltz_api: &'a BoltzApiClientV2,
    ) -> Result<Cooperative<'a>, Error> {
        let signature: Option<(musig::PartialSignature, musig::PublicNonce)> = match boltz_api
            .get_chain_claim_tx_details(swap_id)
            .await
        {
            Ok(claim_tx_response) => {
                if let Some(claim_tx_response) = claim_tx_response {
                    self.validate_cooperative_counterparty(&claim_tx_response.public_key)?;
                    Some(self.script.common().partial_sign(
                        our_refund_keys,
                        &claim_tx_response.pub_nonce,
                        &claim_tx_response.transaction_hash,
                    )?)
                } else {
                    None
                }
            }
            Err(Error::JSON(e)) => {
                log::warn!("Failed to parse chain claim tx details: {e} - continuing without signature as we may have already sent it");
                None
            }
            Err(e) => {
                return Err(e);
            }
        };

        Ok(Cooperative {
            boltz_api,
            swap_id: swap_id.clone(),
            signature,
            swap_auth: None,
        })
    }

    fn validate_cooperative_counterparty(
        &self,
        public_key: &bitcoin::PublicKey,
    ) -> Result<(), Error> {
        let expected = self.script.common().receiver_pubkey();
        if *public_key != expected {
            return Err(Error::Protocol(format!(
                "Cooperative counterparty public key mismatch: {public_key},{expected}"
            )));
        }
        Ok(())
    }

    async fn get_cooperative<'a>(
        &self,
        tx_kind: SwapTxKind,
        options: Option<TransactionOptions>,
        boltz_api: &'a BoltzApiClientV2,
        swap_id: String,
    ) -> Result<Option<Cooperative<'a>>, Error> {
        let o = options.unwrap_or_default();
        match o.cooperative {
            true => match (self.script.common().swap_type(), tx_kind) {
                (SwapType::Chain, SwapTxKind::Claim) => {
                    let claim = o.chain_claim.ok_or(Error::Generic(
                        "Chain claim options are missing".to_string(),
                    ))?;
                    claim
                        .lockup_script
                        .cooperative_chain_claim(&claim.refund_keys, &swap_id, boltz_api)
                        .await
                        .map(Option::Some)
                }
                _ => Ok(Some(Cooperative {
                    boltz_api,
                    swap_id,
                    signature: None,
                    swap_auth: o.swap_auth,
                })),
            },
            false => Ok(None),
        }
    }

    fn check_direct_transaction_inner(
        mrh_amount: Amount,
        network: Network,
        direct_tx: &BtcLikeTransaction,
        claim_address: &str,
        options: DirectTxOptions,
    ) -> Result<(), Error> {
        let amount = match direct_tx {
            BtcLikeTransaction::Bitcoin(_) => {
                Err(Error::Generic("Not implemented for mainchain".to_string()))
            }
            BtcLikeTransaction::Liquid(tx) => {
                let chain = LiquidChain::from(network);
                let address = elements::Address::parse_with_params(claim_address, chain.into())?;
                let script_pubkey = address.script_pubkey();
                let (_, utxo) = super::liquid::find_utxo(tx, &script_pubkey)
                    .ok_or(Error::Generic("No UTXO found for this script".to_string()))?;
                let secp = Secp256k1::new();
                let (value, asset) = match (utxo.value, utxo.asset) {
                    (
                        confidential::Value::Explicit(value),
                        confidential::Asset::Explicit(asset),
                    ) => (value, asset),
                    (
                        confidential::Value::Confidential(_),
                        confidential::Asset::Confidential(_),
                    ) => {
                        let secrets = utxo.unblind(
                            &secp,
                            options.blinding_key.ok_or(Error::Generic(
                                "Blinding key is required for confidential UTXO".to_string(),
                            ))?,
                        )?;
                        (secrets.value, secrets.asset)
                    }
                    (_, _) => {
                        return Err(Error::Generic("Inconsistent blinding".to_string()));
                    }
                };
                if asset != chain.bitcoin() {
                    return Err(Error::Protocol(format!("Asset is not bitcoin: {asset}")));
                }
                Ok(Amount::from_sat(value))
            }
        }?;

        if amount < mrh_amount {
            return Err(Error::Protocol(format!(
                "Amount received via direct transaction is less than expected: {amount} < {mrh_amount}",
            )));
        }
        Ok(())
    }

    pub async fn check_direct_transaction(
        &self,
        chain_client: &ChainClient,
        network: Network,
        direct_tx: &BtcLikeTransaction,
        claim_address: &str,
        options: DirectTxOptions,
    ) -> Result<(), Error> {
        if matches!(
            &self.script,
            SwapScriptImpl::Liquid(script) if script.requires_caller_funded_pset()
        ) {
            return Err(Error::Protocol(
                "Magic routing is L-BTC-only and is disabled for L-USDT".to_string(),
            ));
        }
        chain_client.try_broadcast_tx(direct_tx).await?;
        if let Some(mrh_amount) = self.mrh_amount {
            return SwapScript::check_direct_transaction_inner(
                mrh_amount,
                network,
                direct_tx,
                claim_address,
                options,
            );
        }
        Ok(())
    }

    pub async fn parse_lockup_transaction(
        &self,
        lockup_info: &TransactionInfo,
    ) -> Result<BtcLikeTransaction, Error> {
        let hex = lockup_info
            .hex
            .as_ref()
            .ok_or(Error::Generic("Lockup info is missing".to_string()))?;
        match self.script.clone() {
            SwapScriptImpl::Bitcoin(_) => BtcLikeTransaction::from_hex_bitcoin(hex),
            SwapScriptImpl::Liquid(_) => BtcLikeTransaction::from_hex_liquid(hex),
        }
    }

    fn validate_lockup_amount(&self, amount: Amount) -> Result<(), Error> {
        if let Some(boltz_lockup) = self.boltz_lockup {
            if amount != boltz_lockup {
                return Err(Error::Protocol(format!(
                    "Lockup amount mismatch: {amount} != {boltz_lockup}",
                )));
            }
        }
        Ok(())
    }

    /// Parse the additional outputs for this script's chain up front, so a bad
    /// address fails before anything is fetched, signed or broadcast.
    fn parse_additional_outputs(
        &self,
        chain_client: &ChainClient,
        options: Option<&TransactionOptions>,
    ) -> Result<ParsedAdditionalOutputs, Error> {
        let raw = options
            .map(|options| options.additional_outputs.as_slice())
            .unwrap_or_default();
        Ok(match &self.script {
            SwapScriptImpl::Bitcoin(_) => {
                let network = chain_client.require_bitcoin_client()?.network().into();
                ParsedAdditionalOutputs::Bitcoin(Self::additional_outputs_bitcoin(raw, network)?)
            }
            SwapScriptImpl::Liquid(_) => {
                let network = chain_client.require_liquid_client()?.network().into();
                ParsedAdditionalOutputs::Liquid(Self::additional_outputs_liquid(raw, network)?)
            }
        })
    }

    fn additional_outputs_bitcoin(
        additional_outputs: &[(String, u64)],
        network: bitcoin::Network,
    ) -> Result<Vec<(bitcoin::Address, u64)>, Error> {
        additional_outputs
            .iter()
            .map(|(address, amount)| {
                let address = bitcoin::Address::from_str(address)?;
                if !address.is_valid_for_network(network) {
                    return Err(Error::Address("Address validation failed".to_string()));
                }
                Ok((address.assume_checked(), *amount))
            })
            .collect()
    }

    fn additional_outputs_liquid(
        additional_outputs: &[(String, u64)],
        params: &'static elements::AddressParams,
    ) -> Result<Vec<(elements::Address, u64)>, Error> {
        additional_outputs
            .iter()
            .map(|(address, amount)| {
                Ok((
                    elements::Address::parse_with_params(address, params)?,
                    *amount,
                ))
            })
            .collect()
    }

    /// Build and sign the claim of a reverse or chain swap.
    ///
    /// Before anything that reveals the preimage — the cooperative request
    /// to the maker, or a script-path witness — the lockup output is checked
    /// against the chain client per the options' [`ClaimPolicy`]: it must be
    /// known to the chain source, unspent with no conflicting spend in the
    /// mempool, confirmed to the required depth unless zero-conf was
    /// explicitly allowed, and the tip must still be clear of the swap's
    /// refund timeout by the safety margin. Maker status is never used for
    /// this. A lockup supplied through `with_lockup_tx` and broadcast here is
    /// by definition unconfirmed, so that flow needs `with_allow_zero_conf`.
    pub async fn construct_claim(
        &self,
        preimage: &Preimage,
        params: SwapTransactionParams<'_>,
    ) -> Result<BtcLikeTransaction, Error> {
        self.ensure_uncolored_spend()?;

        let claim_policy = params
            .options
            .as_ref()
            .map(|o| o.claim_policy.clone())
            .unwrap_or_default();
        let additional_outputs =
            self.parse_additional_outputs(params.chain_client, params.options.as_ref())?;
        let cooperative = self
            .get_cooperative(
                SwapTxKind::Claim,
                params.options.clone(),
                params.boltz_api,
                params.swap_id.clone(),
            )
            .await?;
        let lockup_tx = params.options.clone().and_then(|o| o.lockup_tx);
        if params
            .options
            .as_ref()
            .is_none_or(|o| o.broadcast_lockup_tx)
        {
            if let Some(lockup_tx) = lockup_tx.clone() {
                params.chain_client.try_broadcast_tx(&lockup_tx).await?;
            }
        }
        match self.script.clone() {
            SwapScriptImpl::Bitcoin(script) => {
                let chain_client = params.chain_client.require_bitcoin_client()?;

                let utxo = script
                    .fetch_swap_utxo(
                        lockup_tx
                            .as_ref()
                            .map(|tx| {
                                tx.as_bitcoin().ok_or(Error::Generic(
                                    "Lockup transaction is not a Bitcoin transaction".to_string(),
                                ))
                            })
                            .transpose()?,
                        chain_client,
                        params.boltz_api,
                        &params.swap_id,
                        SwapTxKind::Claim,
                    )
                    .await?;

                self.validate_lockup_amount(utxo.1.value)?;
                claim_policy
                    .ensure_bitcoin_claimable(
                        chain_client,
                        &utxo.0,
                        &utxo.1.script_pubkey,
                        script.locktime,
                    )
                    .await?;

                let tx = BtcSwapTx::new_claim_with_utxo(
                    script.as_ref().clone(),
                    params.output_address.clone(),
                    chain_client,
                    utxo,
                )?
                .with_additional_outputs(additional_outputs.into_bitcoin());

                tx.sign_claim(&params.keys, preimage, params.fee, cooperative)
                    .await
                    .map(BtcLikeTransaction::bitcoin)
            }
            SwapScriptImpl::Liquid(script) => {
                let chain_client = params.chain_client.require_liquid_client()?;

                let utxo = script
                    .fetch_swap_utxo(
                        lockup_tx
                            .as_ref()
                            .map(|tx| {
                                tx.as_liquid().ok_or(Error::Generic(
                                    "Lockup transaction is not a Liquid transaction".to_string(),
                                ))
                            })
                            .transpose()?,
                        chain_client,
                        params.boltz_api,
                        &params.swap_id,
                        SwapTxKind::Claim,
                    )
                    .await?;

                if self.boltz_lockup.is_some() {
                    let context = script.resolved_asset_context(chain_client.network());
                    let secrets =
                        decode_swap_output(&utxo.1, script.blinding_secret(), context.swap_asset)?;
                    self.validate_lockup_amount(Amount::from_sat(secrets.value))?;
                }
                claim_policy
                    .ensure_liquid_claimable(
                        chain_client,
                        &utxo.0,
                        &utxo.1.script_pubkey,
                        script.locktime,
                    )
                    .await?;

                let tx = LiquidSwapTx::new_claim_with_utxo(
                    script.as_ref().clone(),
                    params.output_address.clone(),
                    chain_client,
                    utxo,
                )
                .await?
                .with_additional_outputs(additional_outputs.into_liquid());

                tx.sign_claim(&params.keys, preimage, params.fee, cooperative, true)
                    .await
                    .map(BtcLikeTransaction::liquid)
            }
        }
    }

    /// Refuse the plain claim/refund builders for swaps they would get wrong:
    /// an L-USDT HTLC needs a caller-funded fee, and an RGB HTLC spent without
    /// its commitment burns the asset.
    fn ensure_uncolored_spend(&self) -> Result<(), Error> {
        match &self.script {
            SwapScriptImpl::Liquid(script) if script.requires_caller_funded_pset() => Err(
                Error::Protocol("L-USDT spends require the caller-funded PSET flow".to_string()),
            ),
            SwapScriptImpl::Bitcoin(script) => script.ensure_not_rgb(),
            SwapScriptImpl::Liquid(_) => Ok(()),
        }
    }

    /// Prepare the taker's claim of a USDT-RGB reverse swap's lock.
    ///
    /// Run once the lock has `minConfirmations` (the maker sends no
    /// `transaction.confirmed` for it) and the wallet has accepted the lock
    /// consignment. Preparation independently checks chain evidence using the
    /// default [`ClaimPolicy`]. Color and finalize promptly; a prepared template
    /// is not fresh evidence that the lock remains safe to claim. Then finish with
    /// [`PreparedRgbSpend::finalize_claim`]. The claim must confirm before
    /// the swap's timeout block, when the maker's refund becomes valid.
    pub async fn prepare_rgb_claim(
        &self,
        params: RgbPsbtParams<'_>,
    ) -> Result<PreparedRgbSpend, Error> {
        self.prepare_rgb_spend(SwapTxKind::Claim, params, false)
            .await
    }

    /// Prepare the taker's refund of a USDT-RGB submarine swap's lock, valid
    /// from the swap's timeout block on. Finish with
    /// [`PreparedRgbSpend::finalize_refund`].
    pub async fn prepare_rgb_refund(
        &self,
        params: RgbPsbtParams<'_>,
    ) -> Result<PreparedRgbSpend, Error> {
        self.prepare_rgb_spend(SwapTxKind::Refund, params, false)
            .await
    }

    /// Prepare a key-path submarine refund; the maker must advertise RGB cooperation.
    pub async fn prepare_rgb_cooperative_refund(
        &self,
        params: RgbPsbtParams<'_>,
    ) -> Result<PreparedRgbSpend, Error> {
        self.prepare_rgb_spend(SwapTxKind::Refund, params, true)
            .await
    }

    async fn prepare_rgb_spend(
        &self,
        kind: SwapTxKind,
        params: RgbPsbtParams<'_>,
        cooperative: bool,
    ) -> Result<PreparedRgbSpend, Error> {
        let script = match &self.script {
            SwapScriptImpl::Bitcoin(script) if script.is_rgb() => script.as_ref().clone(),
            _ => {
                return Err(Error::Protocol(
                    "Colored RGB spends are only for USDT-RGB swaps".to_string(),
                ))
            }
        };
        let bitcoin_client = params.chain_client.require_bitcoin_client()?;
        let utxo = script
            .find_utxo(&params.lockup_tx, bitcoin_client.network(), kind.clone())?
            .ok_or_else(|| {
                Error::Protocol("No RGB HTLC output in the supplied lock transaction".into())
            })?;
        if kind == SwapTxKind::Claim {
            ClaimPolicy::default()
                .ensure_bitcoin_claimable(
                    bitcoin_client,
                    &utxo.0,
                    &utxo.1.script_pubkey,
                    script.locktime,
                )
                .await?;
        }
        if cooperative {
            return PreparedRgbSpend::new_cooperative_refund(
                script,
                &params.output_address,
                bitcoin_client.network(),
                utxo,
                params.funding,
                params.max_fee,
            );
        }
        PreparedRgbSpend::new(
            kind,
            script,
            &params.output_address,
            bitcoin_client.network(),
            utxo,
            params.funding,
            params.max_fee,
        )
    }

    /// Prepare an L-USDT claim template for caller-provided policy-asset fees.
    pub async fn prepare_liquid_claim(
        &self,
        params: LiquidPsetParams<'_>,
    ) -> Result<PreparedLiquidSpend, Error> {
        self.prepare_liquid_spend(SwapTxKind::Claim, params).await
    }

    /// Prepare an L-USDT refund template for caller-provided policy-asset fees.
    pub async fn prepare_liquid_refund(
        &self,
        params: LiquidPsetParams<'_>,
    ) -> Result<PreparedLiquidSpend, Error> {
        self.prepare_liquid_spend(SwapTxKind::Refund, params).await
    }

    async fn prepare_liquid_spend(
        &self,
        kind: SwapTxKind,
        params: LiquidPsetParams<'_>,
    ) -> Result<PreparedLiquidSpend, Error> {
        let script = match &self.script {
            SwapScriptImpl::Liquid(script) if script.requires_caller_funded_pset() => {
                script.as_ref().clone()
            }
            SwapScriptImpl::Liquid(_) => {
                return Err(Error::Protocol(
                    "Caller-funded PSET spends are not used for L-BTC".to_string(),
                ));
            }
            SwapScriptImpl::Bitcoin(_) => {
                return Err(Error::Protocol(
                    "Caller-funded Liquid PSET requested for a Bitcoin swap".to_string(),
                ));
            }
        };
        if params
            .options
            .as_ref()
            .is_some_and(|options| !options.additional_outputs.is_empty())
        {
            return Err(Error::Protocol(
                "Caller-funded PSET spends do not take additional outputs".to_string(),
            ));
        }
        let liquid_client = params.chain_client.require_liquid_client()?;
        // Pin the payout destination to this swap's chain before anything is
        // broadcast or fetched. `Address::from_str` alone accepts another
        // network's encoding and would silently convert it to a script on the
        // wrong chain, so a mistyped or wrong-network address has to fail here
        // rather than after the invoice is paid.
        let network = liquid_client.network();
        elements::Address::parse_with_params(&params.output_address, network.into()).map_err(
            |_| {
                Error::Protocol(format!(
                    "Liquid payout address is not valid for {network:?}"
                ))
            },
        )?;
        let broadcast_lockup_tx = params
            .options
            .as_ref()
            .is_none_or(|o| o.broadcast_lockup_tx);
        let claim_policy = params
            .options
            .as_ref()
            .map(|o| o.claim_policy.clone())
            .unwrap_or_default();
        let lockup_tx = params.options.and_then(|options| options.lockup_tx);
        if broadcast_lockup_tx {
            if let Some(lockup_tx) = &lockup_tx {
                params.chain_client.try_broadcast_tx(lockup_tx).await?;
            }
        }
        let utxo = script
            .fetch_swap_utxo(
                lockup_tx
                    .as_ref()
                    .map(|tx| {
                        tx.as_liquid().ok_or_else(|| {
                            Error::Generic(
                                "Lockup transaction is not a Liquid transaction".to_string(),
                            )
                        })
                    })
                    .transpose()?,
                liquid_client,
                params.boltz_api,
                &params.swap_id,
                kind.clone(),
            )
            .await?;
        if kind == SwapTxKind::Claim {
            // The finalized claim carries the preimage in its witness, so the
            // same chain evidence is required as for `construct_claim`.
            claim_policy
                .ensure_liquid_claimable(
                    liquid_client,
                    &utxo.0,
                    &utxo.1.script_pubkey,
                    script.locktime,
                )
                .await?;
        }
        let context = script.resolved_asset_context(liquid_client.network());
        let secrets = decode_swap_output(&utxo.1, script.blinding_secret(), context.swap_asset)?;
        if let Some(expected) = self.boltz_lockup {
            self.validate_lockup_amount(Amount::from_sat(secrets.value))?;
            if expected.to_sat() != script.expected_amount {
                return Err(Error::Protocol(
                    "Swap response amount does not match prepared Liquid spend".to_string(),
                ));
            }
        }
        let genesis_hash = liquid_client.get_genesis_hash().await?;
        PreparedLiquidSpend::new(
            kind,
            script,
            &params.output_address,
            utxo.0,
            utxo.1,
            genesis_hash,
            params.max_fee.min(params.quoted_fee_cap),
        )
    }

    pub async fn construct_refund(
        &self,
        params: SwapTransactionParams<'_>,
    ) -> Result<BtcLikeTransaction, Error> {
        self.ensure_uncolored_spend()?;

        let additional_outputs =
            self.parse_additional_outputs(params.chain_client, params.options.as_ref())?;
        let cooperative = self
            .get_cooperative(
                SwapTxKind::Refund,
                params.options,
                params.boltz_api,
                params.swap_id.clone(),
            )
            .await?;

        match self.script.clone() {
            SwapScriptImpl::Bitcoin(script) => {
                let chain_client = params.chain_client.require_bitcoin_client()?;
                let tx = BtcSwapTx::new_refund(
                    script.as_ref().clone(),
                    &params.output_address,
                    chain_client,
                    params.boltz_api,
                    params.swap_id.clone(),
                )
                .await?
                .with_additional_outputs(additional_outputs.into_bitcoin());
                tx.sign_refund(&params.keys, params.fee, cooperative)
                    .await
                    .map(BtcLikeTransaction::bitcoin)
            }
            SwapScriptImpl::Liquid(script) => {
                let chain_client = params.chain_client.require_liquid_client()?;
                let tx = LiquidSwapTx::new_refund(
                    script.as_ref().clone(),
                    &params.output_address,
                    chain_client,
                    params.boltz_api,
                    params.swap_id.clone(),
                )
                .await?
                .with_additional_outputs(additional_outputs.into_liquid());
                tx.sign_refund(&params.keys, params.fee, cooperative, true)
                    .await
                    .map(BtcLikeTransaction::liquid)
            }
        }
    }
}

enum ParsedAdditionalOutputs {
    Bitcoin(Vec<(bitcoin::Address, u64)>),
    Liquid(Vec<(elements::Address, u64)>),
}

impl ParsedAdditionalOutputs {
    fn into_bitcoin(self) -> Vec<(bitcoin::Address, u64)> {
        match self {
            Self::Bitcoin(outputs) => outputs,
            Self::Liquid(_) => Vec::new(),
        }
    }

    fn into_liquid(self) -> Vec<(elements::Address, u64)> {
        match self {
            Self::Liquid(outputs) => outputs,
            Self::Bitcoin(_) => Vec::new(),
        }
    }
}

/// Proof-of-payment gate for cooperative submarine claims: the preimage the
/// server returns must hash to the payment hash committed in the invoice.
/// Partial-signing without this check would let the server key-path-sweep
/// the lockup without ever paying the invoice.
pub fn verify_submarine_preimage(invoice: &str, preimage_hex: &str) -> Result<(), Error> {
    let preimage = Preimage::from_str(preimage_hex)?;
    let invoice = LightningInvoice::from_str(invoice)?;
    if invoice.payment_hash() != preimage.sha256.to_string() {
        return Err(Error::Protocol(
            "Preimage does not match invoice payment hash".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::sha256;
    use bitcoin::{
        absolute::LockTime,
        hashes::{hash160, Hash},
        PublicKey,
    };
    use elements::{
        confidential::Value, OutPoint, Script, Sequence, Transaction, TxIn, TxInWitness, TxOut,
        TxOutWitness,
    };

    // Helper function to generate a regtest address
    fn generate_regtest_address() -> String {
        use bitcoin::key::rand::thread_rng;
        let network = Network::Regtest;
        let chain = LiquidChain::from(network);
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let keypair = bitcoin::secp256k1::Keypair::new(&secp, &mut thread_rng());

        let addr = elements::Address::p2wpkh(
            &elements::bitcoin::PublicKey::new(keypair.public_key()),
            None,
            chain.into(),
        );

        addr.to_string()
    }

    // Helper function to create a Liquid transaction with explicit value
    fn create_liquid_tx_explicit(address: &str, amount: u64) -> BtcLikeTransaction {
        let network = Network::Regtest;
        let chain = LiquidChain::from(network);
        let addr = elements::Address::parse_with_params(address, chain.into()).unwrap();
        let script_pubkey = addr.script_pubkey();

        // Use the regtest LBTC asset
        let asset = chain.bitcoin();

        let tx = Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::default(),
                is_pegin: false,
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                asset_issuance: elements::AssetIssuance::default(),
                witness: TxInWitness::default(),
            }],
            output: vec![
                TxOut {
                    asset: confidential::Asset::Explicit(asset),
                    value: Value::Explicit(amount),
                    nonce: confidential::Nonce::Null,
                    script_pubkey: script_pubkey.clone(),
                    witness: TxOutWitness::default(),
                },
                // Add a fee output
                TxOut {
                    asset: confidential::Asset::Explicit(asset),
                    value: Value::Explicit(1000),
                    nonce: confidential::Nonce::Null,
                    script_pubkey: Script::new(),
                    witness: TxOutWitness::default(),
                },
            ],
        };

        BtcLikeTransaction::Liquid(tx)
    }

    /// An L-USDT claim must reject a payout address from another Liquid network
    /// before it touches the chain, otherwise the mistake only surfaces after
    /// the hold invoice has been paid.
    #[tokio::test]
    async fn prepare_liquid_claim_rejects_a_wrong_network_payout_address() {
        use crate::network::esplora::EsploraLiquidClient;
        use crate::swaps::liquid::LiquidAssetContext;
        use crate::util::secrets::Preimage;
        use bitcoin::key::rand::rngs::OsRng;

        let chain = LiquidChain::LiquidRegtest;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let keys = bitcoin::secp256k1::Keypair::new(&secp, &mut OsRng);
        let script = SwapScript::new(
            SwapScriptImpl::liquid(LiquidSwapScript {
                swap_type: SwapType::ReverseSubmarine,
                side: None,
                funding_addrs: None,
                hashlock: Preimage::random().hash160,
                receiver_pubkey: bitcoin::PublicKey::new(keys.public_key()),
                locktime: elements::LockTime::from_height(200).unwrap(),
                sender_pubkey: bitcoin::PublicKey::new(keys.public_key()),
                blinding_key: None,
                asset_context: Some(LiquidAssetContext {
                    swap_asset: elements::AssetId::from_str(
                        "1111111111111111111111111111111111111111111111111111111111111111",
                    )
                    .unwrap(),
                    policy_asset: chain.bitcoin(),
                }),
                expected_amount: 100_000,
            }),
            None,
            None,
        );
        // Guard the premise: this is the caller-funded PSET flow.
        assert!(
            matches!(&script.script, SwapScriptImpl::Liquid(inner) if inner.requires_caller_funded_pset())
        );

        let chain_client = ChainClient::new().with_liquid(EsploraLiquidClient::new(
            chain,
            "http://127.0.0.1:1/api",
            1,
        ));
        let boltz_api = BoltzApiClientV2::new("http://127.0.0.1:1/v2".to_string(), None);
        // A mainnet address, offered to a regtest swap.
        let mainnet_address = elements::Address::p2wpkh(
            &elements::bitcoin::PublicKey::new(keys.public_key()),
            None,
            LiquidChain::Liquid.into(),
        )
        .to_string();

        let error = script
            .prepare_liquid_claim(LiquidPsetParams {
                output_address: mainnet_address,
                max_fee: 1_000,
                quoted_fee_cap: 1_000,
                swap_id: "swap".to_string(),
                chain_client: &chain_client,
                boltz_api: &boltz_api,
                options: None,
            })
            .await
            .expect_err("a mainnet payout address must not be accepted for a regtest swap");
        assert!(
            error
                .to_string()
                .contains("Liquid payout address is not valid for"),
            "rejected for the wrong reason: {error}"
        );

        // The matching regtest address gets past the address check and only
        // then fails on the unreachable Esplora backend.
        let error = script
            .prepare_liquid_claim(LiquidPsetParams {
                output_address: generate_regtest_address(),
                max_fee: 1_000,
                quoted_fee_cap: 1_000,
                swap_id: "swap".to_string(),
                chain_client: &chain_client,
                boltz_api: &boltz_api,
                options: None,
            })
            .await
            .expect_err("no backend is listening");
        assert!(
            !error
                .to_string()
                .contains("Liquid payout address is not valid for"),
            "a same-network address must pass the address check: {error}"
        );
    }

    #[test]
    fn cooperative_counterparty_rejects_unexpected_key() {
        use bitcoin::key::rand::thread_rng;

        let secp = Secp256k1::new();
        let receiver_keys = Keypair::new(&secp, &mut thread_rng());
        let sender_keys = Keypair::new(&secp, &mut thread_rng());
        let unexpected_keys = Keypair::new(&secp, &mut thread_rng());
        let public_key = |keys: &Keypair| PublicKey {
            compressed: true,
            inner: keys.public_key(),
        };
        let script = SwapScript::new(
            SwapScriptImpl::bitcoin(BtcSwapScript {
                swap_type: SwapType::Submarine,
                side: None,
                funding_addrs: None,
                hashlock: hash160::Hash::hash(&[0; 32]),
                receiver_pubkey: public_key(&receiver_keys),
                locktime: LockTime::from_consensus(200),
                sender_pubkey: public_key(&sender_keys),
                expected_amount: 0,
                rgb: None,
            }),
            None,
            None,
        );

        assert!(script
            .validate_cooperative_counterparty(&public_key(&unexpected_keys))
            .is_err());
    }

    #[test]
    fn test_check_direct_transaction_bitcoin_not_implemented() {
        let btc_tx = BtcLikeTransaction::Bitcoin(bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        });

        let result = SwapScript::check_direct_transaction_inner(
            Amount::from_sat(100_000),
            Network::Regtest,
            &btc_tx,
            "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080",
            DirectTxOptions::new(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Not implemented for mainchain"));
    }

    #[test]
    fn test_check_direct_transaction_liquid_explicit_success() {
        let address = generate_regtest_address();
        let amount = 100_000;

        let liquid_tx = create_liquid_tx_explicit(&address, amount);

        let result = SwapScript::check_direct_transaction_inner(
            Amount::from_sat(amount),
            Network::Regtest,
            &liquid_tx,
            &address,
            DirectTxOptions::new(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_check_direct_transaction_liquid_wrong_asset() {
        let address = generate_regtest_address();
        let amount = 100_000;

        // Create a transaction with the wrong asset
        let network = Network::Regtest;
        let chain = LiquidChain::from(network);
        let addr = elements::Address::parse_with_params(&address, chain.into()).unwrap();
        let script_pubkey = addr.script_pubkey();

        // Use a wrong asset ID (just some random asset, not the regtest LBTC)
        let wrong_asset = elements::AssetId::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();

        let tx = Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::default(),
                is_pegin: false,
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                asset_issuance: elements::AssetIssuance::default(),
                witness: TxInWitness::default(),
            }],
            output: vec![
                TxOut {
                    asset: confidential::Asset::Explicit(wrong_asset),
                    value: Value::Explicit(amount),
                    nonce: confidential::Nonce::Null,
                    script_pubkey: script_pubkey.clone(),
                    witness: TxOutWitness::default(),
                },
                // Add a fee output
                TxOut {
                    asset: confidential::Asset::Explicit(wrong_asset),
                    value: Value::Explicit(1000),
                    nonce: confidential::Nonce::Null,
                    script_pubkey: Script::new(),
                    witness: TxOutWitness::default(),
                },
            ],
        };

        let liquid_tx = BtcLikeTransaction::Liquid(tx);

        let result = SwapScript::check_direct_transaction_inner(
            Amount::from_sat(amount),
            Network::Regtest,
            &liquid_tx,
            &address,
            DirectTxOptions::new(),
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("asset") || err_msg.contains("Asset"));
    }

    #[test]
    fn test_check_direct_transaction_liquid_no_utxo_found() {
        let tx_address = generate_regtest_address();
        let different_address = generate_regtest_address();

        let liquid_tx = create_liquid_tx_explicit(&tx_address, 100_000);

        let result = SwapScript::check_direct_transaction_inner(
            Amount::from_sat(100_000),
            Network::Regtest,
            &liquid_tx,
            &different_address,
            DirectTxOptions::new(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No UTXO found for this script"));
    }

    #[test]
    fn test_check_direct_transaction_amount_validation_less_than_expected() {
        let expected_amount = Amount::from_sat(100_000);
        let address = generate_regtest_address();
        let actual_amount = 50_000; // Less than expected

        let liquid_tx = create_liquid_tx_explicit(&address, actual_amount);

        let result = SwapScript::check_direct_transaction_inner(
            expected_amount,
            Network::Regtest,
            &liquid_tx,
            &address,
            DirectTxOptions::new(),
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Amount received via direct transaction is less than expected"));
    }

    #[test]
    fn test_check_direct_transaction_amount_validation_equal() {
        let expected_amount = Amount::from_sat(100_000);
        let address = generate_regtest_address();

        let liquid_tx = create_liquid_tx_explicit(&address, expected_amount.to_sat());

        let result = SwapScript::check_direct_transaction_inner(
            expected_amount,
            Network::Regtest,
            &liquid_tx,
            &address,
            DirectTxOptions::new(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_check_direct_transaction_amount_validation_greater() {
        let expected_amount = Amount::from_sat(100_000);
        let address = generate_regtest_address();
        let actual_amount = 150_000; // Greater than expected

        let liquid_tx = create_liquid_tx_explicit(&address, actual_amount);

        let result = SwapScript::check_direct_transaction_inner(
            expected_amount,
            Network::Regtest,
            &liquid_tx,
            &address,
            DirectTxOptions::new(),
        );

        assert!(result.is_ok());
    }

    fn signed_test_invoice(payment_hash: sha256::Hash) -> String {
        use lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};

        let secp = Secp256k1::new();
        let key = bitcoin::secp256k1::SecretKey::from_slice(&[41u8; 32]).unwrap();
        InvoiceBuilder::new(Currency::Bitcoin)
            .description("verify_submarine_preimage test".to_string())
            .payment_hash(payment_hash)
            .payment_secret(PaymentSecret([7u8; 32]))
            .duration_since_epoch(std::time::Duration::from_secs(1_726_000_000))
            .min_final_cltv_expiry_delta(144)
            .build_signed(|hash| secp.sign_ecdsa_recoverable(hash, &key))
            .unwrap()
            .to_string()
    }

    #[test]
    fn submarine_preimage_matching_invoice_is_accepted() {
        let preimage = [42u8; 32];
        let invoice = signed_test_invoice(sha256::Hash::hash(&preimage));
        let preimage_hex = preimage
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        assert!(verify_submarine_preimage(&invoice, &preimage_hex).is_ok());
    }

    #[test]
    fn submarine_preimage_mismatch_is_rejected() {
        let preimage = [42u8; 32];
        let invoice = signed_test_invoice(sha256::Hash::hash(&preimage));
        let wrong = [43u8; 32]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        assert!(matches!(
            verify_submarine_preimage(&invoice, &wrong),
            Err(Error::Protocol(_))
        ));
    }

    #[test]
    fn submarine_preimage_bad_hex_is_rejected() {
        let preimage = [42u8; 32];
        let invoice = signed_test_invoice(sha256::Hash::hash(&preimage));

        assert!(verify_submarine_preimage(&invoice, "not-hex").is_err());
    }

    /// A Bitcoin chain source that answers tip and output-status queries from
    /// fixed values, fails everything else, and records every call. Lets the
    /// claim guard be driven through `construct_claim` without a network.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    struct FakeBitcoin {
        tip: u32,
        status: Option<OutputStatus>,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    impl FakeBitcoin {
        fn new(tip: u32, status: Option<OutputStatus>) -> Self {
            Self {
                tip,
                status,
                calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
        fn record(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_string());
        }
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[macros::async_trait]
    impl BitcoinClient for FakeBitcoin {
        async fn get_address_balance(
            &self,
            _address: &bitcoin::Address,
        ) -> Result<(u64, i64), Error> {
            self.record("get_address_balance");
            Err(Error::Generic("unexpected".into()))
        }
        async fn get_address_utxos(
            &self,
            _address: &bitcoin::Address,
        ) -> Result<Vec<(bitcoin::OutPoint, bitcoin::TxOut)>, Error> {
            self.record("get_address_utxos");
            Err(Error::Generic("unexpected".into()))
        }
        async fn get_tx(&self, _txid: bitcoin::Txid) -> Result<bitcoin::Transaction, Error> {
            self.record("get_tx");
            Err(Error::Generic("unexpected".into()))
        }
        async fn broadcast_tx(
            &self,
            _signed_tx: &bitcoin::Transaction,
        ) -> Result<bitcoin::Txid, Error> {
            self.record("broadcast_tx");
            Err(Error::Generic("unexpected".into()))
        }
        async fn get_tip_height(&self) -> Result<u32, Error> {
            self.record("get_tip_height");
            Ok(self.tip)
        }
        async fn get_output_status(
            &self,
            _outpoint: &bitcoin::OutPoint,
            _script_pubkey: &bitcoin::Script,
        ) -> Result<Option<OutputStatus>, Error> {
            self.record("get_output_status");
            Ok(self.status)
        }
        fn network(&self) -> crate::network::BitcoinChain {
            crate::network::BitcoinChain::BitcoinRegtest
        }
    }

    /// A reverse swap script with timeout 200 and its lockup transaction,
    /// whose swap output sits behind an unrelated one.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn reverse_fixture() -> (
        SwapScript,
        Keypair,
        crate::util::secrets::Preimage,
        bitcoin::Transaction,
        u64,
    ) {
        use crate::util::secrets::Preimage;

        let secp = Secp256k1::new();
        let our_keys = Keypair::from_seckey_slice(&secp, &[0x11; 32]).unwrap();
        let maker_keys = Keypair::from_seckey_slice(&secp, &[0x22; 32]).unwrap();
        let preimage = Preimage::from_vec(vec![0x33; 32]).unwrap();
        let amount = 50_000;
        let btc_script = BtcSwapScript {
            swap_type: SwapType::ReverseSubmarine,
            side: None,
            funding_addrs: None,
            hashlock: preimage.hash160,
            receiver_pubkey: PublicKey::new(our_keys.public_key()),
            locktime: LockTime::from_height(200).unwrap(),
            sender_pubkey: PublicKey::new(maker_keys.public_key()),
            expected_amount: amount,
            rgb: None,
        };
        let chain = crate::network::BitcoinChain::BitcoinRegtest;
        let swap_spk = btc_script.to_address(chain).unwrap().script_pubkey();
        let script = SwapScript::new(
            SwapScriptImpl::bitcoin(btc_script),
            Some(Amount::from_sat(amount)),
            None,
        );
        // An unrelated output first, so a claim that picked by position
        // rather than by script would spend the wrong one.
        let lockup = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: bitcoin::OutPoint::new(
                    bitcoin::Txid::from_byte_array([0x44; 32]),
                    0,
                ),
                ..Default::default()
            }],
            output: vec![
                bitcoin::TxOut {
                    value: Amount::from_sat(70_000),
                    script_pubkey: bitcoin::ScriptBuf::new_p2wpkh(
                        &bitcoin::WPubkeyHash::from_byte_array([0x55; 20]),
                    ),
                },
                bitcoin::TxOut {
                    value: Amount::from_sat(amount),
                    script_pubkey: swap_spk,
                },
            ],
        };
        (script, our_keys, preimage, lockup, amount)
    }

    /// Script-path claim of the fixture against `chain`, pinned to the
    /// existing lockup. Nothing listens at the maker URL: any maker request
    /// fails the claim.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    async fn claim_fixture(
        tip: u32,
        status: Option<OutputStatus>,
        options: TransactionOptions,
    ) -> Result<bitcoin::Transaction, Error> {
        let (script, our_keys, preimage, lockup, _) = reverse_fixture();
        let lockup_hex = bitcoin::consensus::encode::serialize_hex(&lockup);
        let chain_client = ChainClient::new().with_bitcoin(FakeBitcoin::new(tip, status));
        let boltz_api = BoltzApiClientV2::new("http://127.0.0.1:1/v2".to_string(), None);
        let payout = bitcoin::Address::p2wpkh(
            &bitcoin::CompressedPublicKey(our_keys.public_key()),
            bitcoin::Network::Regtest,
        );
        let claim = script
            .construct_claim(
                &preimage,
                SwapTransactionParams {
                    keys: our_keys,
                    output_address: payout.to_string(),
                    fee: Fee::Absolute(500),
                    swap_id: "reverse".to_string(),
                    chain_client: &chain_client,
                    boltz_api: &boltz_api,
                    options: Some(options.with_cooperative(false).with_existing_lockup_tx(
                        BtcLikeTransaction::from_hex_bitcoin(&lockup_hex).unwrap(),
                    )),
                },
            )
            .await?;
        Ok(claim.as_bitcoin().unwrap().clone())
    }

    fn confirmed(height: u32) -> Option<OutputStatus> {
        Some(OutputStatus {
            block_height: Some(height),
            spend: OutputSpend::Unspent,
        })
    }

    /// A claim pinned to an existing lockup spends that transaction's swap
    /// output, picked by script rather than position. It neither rebroadcasts
    /// the lockup nor asks the chain source or the maker where the lockup is;
    /// it asks the chain source only what it must know before revealing the
    /// preimage: the tip and the output's status.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn btc_claim_spends_the_existing_lockup_tx() {
        let (script, our_keys, preimage, lockup, amount) = reverse_fixture();
        let lockup_txid = lockup.compute_txid();
        let lockup_hex = bitcoin::consensus::encode::serialize_hex(&lockup);
        let fake = FakeBitcoin::new(150, confirmed(150));
        let calls = fake.calls.clone();
        let chain_client = ChainClient::new().with_bitcoin(fake);
        let boltz_api = BoltzApiClientV2::new("http://127.0.0.1:1/v2".to_string(), None);
        let payout = bitcoin::Address::p2wpkh(
            &bitcoin::CompressedPublicKey(our_keys.public_key()),
            bitcoin::Network::Regtest,
        );

        let claim = script
            .construct_claim(
                &preimage,
                SwapTransactionParams {
                    keys: our_keys,
                    output_address: payout.to_string(),
                    fee: Fee::Absolute(500),
                    swap_id: "reverse".to_string(),
                    chain_client: &chain_client,
                    boltz_api: &boltz_api,
                    options: Some(
                        TransactionOptions::default()
                            .with_cooperative(false)
                            .with_existing_lockup_tx(
                                BtcLikeTransaction::from_hex_bitcoin(&lockup_hex).unwrap(),
                            ),
                    ),
                },
            )
            .await
            .unwrap();

        let claim = claim.as_bitcoin().unwrap();
        assert_eq!(claim.input.len(), 1);
        assert_eq!(
            claim.input[0].previous_output,
            bitcoin::OutPoint::new(lockup_txid, 1)
        );
        assert_eq!(claim.output[0].value, Amount::from_sat(amount - 500));
        assert_eq!(claim.output[0].script_pubkey, payout.script_pubkey());
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "get_tip_height".to_string(),
                "get_output_status".to_string()
            ]
        );
    }

    /// The default policy refuses to reveal the preimage against a lockup the
    /// chain source has not confirmed, has never seen, or sees a spend of.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn btc_claim_refuses_unconfirmed_unknown_and_spent_lockups() {
        let error = claim_fixture(
            150,
            Some(OutputStatus {
                block_height: None,
                spend: OutputSpend::Unspent,
            }),
            TransactionOptions::default(),
        )
        .await
        .unwrap_err();
        assert!(error.message().contains("0 confirmations"), "{error}");

        let error = claim_fixture(150, None, TransactionOptions::default())
            .await
            .unwrap_err();
        assert!(
            error.message().contains("unknown to the chain source"),
            "{error}"
        );

        let mempool_spend = Some(OutputStatus {
            block_height: Some(150),
            spend: OutputSpend::SpentUnconfirmed,
        });
        let error = claim_fixture(150, mempool_spend, TransactionOptions::default())
            .await
            .unwrap_err();
        assert!(error.message().contains("conflicting spend"), "{error}");
        // Even an explicit zero-conf opt-in does not claim over a spend.
        let error = claim_fixture(
            150,
            mempool_spend,
            TransactionOptions::default().with_allow_zero_conf(true),
        )
        .await
        .unwrap_err();
        assert!(error.message().contains("conflicting spend"), "{error}");
        // Fee-bumping one's own stuck claim is the explicit exception...
        claim_fixture(
            150,
            mempool_spend,
            TransactionOptions::default().with_allow_conflicting_spend(true),
        )
        .await
        .unwrap();
        // ...and it never extends to a spend that is already in a block.
        let confirmed_spend = Some(OutputStatus {
            block_height: Some(150),
            spend: OutputSpend::SpentConfirmed,
        });
        let error = claim_fixture(
            150,
            confirmed_spend,
            TransactionOptions::default().with_allow_conflicting_spend(true),
        )
        .await
        .unwrap_err();
        assert!(error.message().contains("already spent"), "{error}");
    }

    /// Zero-conf is an explicit opt-in: with it, an unconfirmed or not yet
    /// indexed lockup is claimed; without it `min_confirmations` is enforced
    /// against the tip.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn btc_claim_zero_conf_is_opt_in_and_depth_is_enforced() {
        claim_fixture(
            150,
            Some(OutputStatus {
                block_height: None,
                spend: OutputSpend::Unspent,
            }),
            TransactionOptions::default().with_allow_zero_conf(true),
        )
        .await
        .unwrap();
        claim_fixture(
            150,
            None,
            TransactionOptions::default().with_allow_zero_conf(true),
        )
        .await
        .unwrap();

        // Confirmed in the tip block: one confirmation.
        claim_fixture(
            150,
            confirmed(150),
            TransactionOptions::default().with_min_confirmations(1),
        )
        .await
        .unwrap();
        let error = claim_fixture(
            150,
            confirmed(150),
            TransactionOptions::default().with_min_confirmations(2),
        )
        .await
        .unwrap_err();
        assert!(
            error.message().contains("1 confirmations, 2 required"),
            "{error}"
        );
        claim_fixture(
            151,
            confirmed(150),
            TransactionOptions::default().with_min_confirmations(2),
        )
        .await
        .unwrap();
        // min_confirmations = 0 is only meaningful with the explicit opt-in.
        let error = claim_fixture(
            151,
            confirmed(150),
            TransactionOptions::default().with_min_confirmations(0),
        )
        .await
        .unwrap_err();
        assert!(error.message().contains("min_confirmations"), "{error}");
    }

    /// The fixture's timeout is block 200. With the Bitcoin default margin of
    /// 6, tip 194 reaches the timeout and is refused; tip 193 is not. The
    /// margin is adjustable, and zero accepts the race up to the timeout.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[tokio::test]
    async fn btc_claim_refuses_to_run_into_the_timeout() {
        let error = claim_fixture(194, confirmed(100), TransactionOptions::default())
            .await
            .unwrap_err();
        assert!(
            error.message().contains("Too close to the swap timeout"),
            "{error}"
        );
        let error = claim_fixture(
            194,
            confirmed(100),
            TransactionOptions::default().with_allow_zero_conf(true),
        )
        .await
        .unwrap_err();
        assert!(
            error.message().contains("Too close to the swap timeout"),
            "{error}"
        );

        claim_fixture(193, confirmed(100), TransactionOptions::default())
            .await
            .unwrap();

        claim_fixture(
            194,
            confirmed(100),
            TransactionOptions::default().with_claim_safety_margin_blocks(0),
        )
        .await
        .unwrap();
        let error = claim_fixture(
            200,
            confirmed(100),
            TransactionOptions::default().with_claim_safety_margin_blocks(0),
        )
        .await
        .unwrap_err();
        assert!(
            error.message().contains("Too close to the swap timeout"),
            "{error}"
        );
    }

    /// The chain-level decision, without a chain: Liquid's default margin is
    /// 60 blocks, a time-based timeout cannot be checked and is refused, and
    /// the checks fail closed in the order a caller would want to hear them.
    #[macros::test_all]
    fn claim_policy_check_covers_both_chains() {
        let btc = Chain::Bitcoin(crate::network::BitcoinChain::BitcoinRegtest);
        let lbtc = Chain::Liquid(LiquidChain::LiquidRegtest);
        let policy = ClaimPolicy::default();
        assert!(policy
            .check(btc, 193, confirmed(100), Some(200), "o")
            .is_ok());
        assert!(policy
            .check(btc, 194, confirmed(100), Some(200), "o")
            .is_err());
        // Liquid: 60 blocks of margin by default.
        assert!(policy
            .check(lbtc, 139, confirmed(100), Some(200), "o")
            .is_ok());
        assert!(policy
            .check(lbtc, 140, confirmed(100), Some(200), "o")
            .is_err());
        let margin = ClaimPolicy {
            claim_safety_margin_blocks: Some(6),
            ..ClaimPolicy::default()
        };
        assert!(margin
            .check(lbtc, 193, confirmed(100), Some(200), "o")
            .is_ok());
        // No block-height deadline: refused regardless of policy.
        let zero_conf = ClaimPolicy {
            allow_zero_conf: true,
            ..ClaimPolicy::default()
        };
        assert!(zero_conf.check(btc, 10, confirmed(5), None, "o").is_err());
        assert!(zero_conf.check(btc, 10, None, Some(200), "o").is_ok());
    }
}
