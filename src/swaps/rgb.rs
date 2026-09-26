//! RGB assets on Bitcoin L1 (USDT-RGB): the taker's half of the maker's RGB
//! routes.
//!
//! The maker serves three RGB routes, all on the Bitcoin chain:
//!
//! - **Submarine** `USDT-RGB → BTC` over Lightning. The taker locks the asset
//!   in the swap tree's P2TR output with an rgb-lib `send` to a witness
//!   recipient, and the maker pays the invoice once the lock is confirmed.
//!   After the timeout an unclaimed lock is the taker's to refund through the
//!   refund leaf.
//! - **Reverse** `BTC → USDT-RGB` over Lightning. The maker locks the asset
//!   in the swap tree once the hold invoice is paid, and the taker claims it
//!   through the claim leaf. The HTLC carries the claim's fee, so the taker
//!   needs no BTC of their own.
//! - **Atomic** `BTC ⇄ USDT-RGB`, one on-chain transaction built by rgb-lib's
//!   swap protocol and relayed through `/v2/swap/atomic`.
//!
//! The HTLC is the ordinary Boltz swap tree ([`BtcSwapScript`]); what RGB
//! changes is how it is funded and spent. Both happen in the taker's rgb-lib
//! wallet, which this crate does not link. This module is the rest: the wire
//! types, the checks a taker makes before it locks or waits on a lock
//! ([`RgbLock`], [`RgbLockExpectations`]), and the claim and refund
//! transactions ([`RgbHtlcSpend`]), built for rgb-lib to color and signed by
//! the swap key once colored.
//!
//! **A spend of an RGB HTLC without an RGB commitment burns the asset.** The
//! transaction [`RgbHtlcSpend::psbt`] returns has an empty OP_RETURN at output
//! 0 that rgb-lib's `psbt_op_prepare` writes the commitment into, and
//! [`RgbHtlcSpend::sign_colored`] refuses a PSBT whose output 0 does not carry
//! one. Never spend the HTLC with [`super::bitcoin::BtcSwapTx`]: it builds an
//! uncolored spend.
//!
//! **Amounts.** Every RGB amount the maker states — `expectedAmount`,
//! `onchainAmount`, `rgb.amount`, pair limits, fees, `rate` and the atomic
//! quote — counts the asset's contract units, at the contract's own precision
//! (6 decimals for USDT-RGB), not sats and not 8-decimal card units.

use std::str::FromStr;

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Keypair, Message, Secp256k1};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::taproot::{LeafVersion, Signature};
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, Psbt, ScriptBuf, Sequence, TapLeafHash, TapSighashType, Transaction,
    TxIn, TxOut, Weight, Witness,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bitcoin::BtcSwapScript;
use super::boltz::{
    BoltzApiClientV2, CreateReverseRequest, CreateReverseResponse, CreateSubmarineRequest,
    CreateSubmarineResponse, SwapType,
};
use crate::error::Error;
use crate::network::{BitcoinChain, Chain};
use crate::util::secrets::Preimage;

/// The maker's currency name for Tether on RGB, in pair keys and on the wire.
pub const USDT_RGB: &str = "USDT-RGB";

/// Atomic pair: the taker sends BTC and receives USDT-RGB.
pub const BTC_TO_USDT_RGB: &str = "BTC/USDT-RGB";

/// Atomic pair: the taker sends USDT-RGB and receives BTC.
pub const USDT_RGB_TO_BTC: &str = "USDT-RGB/BTC";

/// Every RGB contract id starts with this.
pub const RGB_ASSET_ID_PREFIX: &str = "rgb:";

/// Smallest output of an HTLC spend, the colored destination and any change:
/// above P2TR dust (330) with room to spare. The maker uses the same floor.
pub const MIN_HTLC_OUTPUT_SAT: u64 = 546;

/// The largest `htlcSat` [`RgbLockExpectations::default`] lets a submarine
/// ask the taker to lock. The maker claims those sats with the asset, so an
/// unbounded value would let a maker take BTC the quote never mentioned. The
/// maker's default is 1 000.
pub const DEFAULT_MAX_SUBMARINE_HTLC_SAT: u64 = 10_000;

/// Sequence of an HTLC spend input: RBF-signalling, no relative timelock. On
/// a refund it also keeps `nLockTime` enforced.
pub const HTLC_SPEND_SEQUENCE: Sequence = Sequence::ENABLE_RBF_NO_LOCKTIME;

/// Output of an [`RgbHtlcSpend`] that receives the asset: the index to put
/// in rgb-lib's `AssetColoringInfo::output_map`.
pub const COLORED_OUTPUT_INDEX: u32 = 1;

/// Witness weight of a P2TR key-path input (item count + 64-byte signature):
/// the fee input an rgb-lib wallet adds to a refund.
const KEY_PATH_WITNESS_WEIGHT: u64 = 1 + 1 + 64;

/// Bytes the OP_RETURN grows by once rgb-lib writes the 32-byte commitment
/// into it (`OP_RETURN` → `OP_RETURN OP_PUSHBYTES_32 <commitment>`).
const COMMITMENT_BYTES: u64 = 33;

/// Control block of a two-leaf swap tree: 33-byte header + one sibling node.
const CONTROL_BLOCK_BYTES: u64 = 33 + 32;

fn compact_size(n: u64) -> u64 {
    match n {
        0..=0xfc => 1,
        0xfd..=0xffff => 3,
        _ => 5,
    }
}

/// Weight of a claim-leaf witness `[sig, preimage, script, control block]`
/// for a claim leaf of `script_len` bytes.
pub fn claim_leaf_witness_weight(script_len: u64) -> u64 {
    1 + (1 + 64) + (1 + 32) + (compact_size(script_len) + script_len) + (1 + CONTROL_BLOCK_BYTES)
}

/// Weight of a refund-leaf witness `[sig, script, control block]` for a
/// refund leaf of `script_len` bytes.
pub fn refund_leaf_witness_weight(script_len: u64) -> u64 {
    1 + (1 + 64) + (compact_size(script_len) + script_len) + (1 + CONTROL_BLOCK_BYTES)
}

/// The sats a reverse lock must carry for the taker's claim to pay its own
/// fee at `fee_rate_sat_vb` and still leave a [`MIN_HTLC_OUTPUT_SAT`] colored
/// output: what the maker funds `htlcSat` with.
pub fn taker_claim_htlc_sat(claim_witness_weight: u64, fee_rate_sat_vb: u64) -> Result<u64, Error> {
    let tx = spend_tx(
        LockTime::ZERO,
        vec![OutPoint::null()],
        vec![
            op_return_placeholder(),
            TxOut {
                value: Amount::ZERO,
                script_pubkey: p2tr_placeholder(),
            },
        ],
    );
    spend_fee(&tx, claim_witness_weight, fee_rate_sat_vb)?
        .checked_add(MIN_HTLC_OUTPUT_SAT)
        .ok_or_else(|| Error::Protocol("RGB HTLC sats overflow".to_string()))
}

fn rgb_error(message: impl Into<String>) -> Error {
    Error::Protocol(format!("RGB: {}", message.into()))
}

/// The `rgb` object of a create response: how the asset is locked in the
/// swap tree's P2TR output.
///
/// **Submarine** (the taker locks): the taker's rgb-lib `send` pays a witness
/// recipient — `recipientId` with `WitnessData { amount_sat: htlcSat,
/// blinding: Some(blinding) }` — assigning `amount` of `assetId`, and posts
/// the consignment to `transportEndpoints`. The maker pays the invoice once
/// the lock has `minConfirmations` and holds exactly `amount`.
///
/// **Reverse** (the maker locks): once the hold invoice is paid, the maker
/// sends `htlcSat` to `scriptPubkey` assigning `amount`, posting the
/// consignment under `recipientId`; the lock txid is then on
/// `GET /v2/swap/reverse/{id}/transaction`. The taker accepts the transfer
/// with `blinding` and claims it with an [`RgbHtlcSpend::claim`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbLock {
    /// RGB contract id (`rgb:…`).
    pub asset_id: String,
    /// Asset amount in the contract's smallest unit; equals `expectedAmount`
    /// (submarine) or `onchainAmount` (reverse).
    pub amount: u64,
    /// rgb-lib recipient id of the HTLC script: the proxy key of the lock
    /// consignment.
    pub recipient_id: String,
    /// The blinding the lock uses, chosen by the maker.
    pub blinding: u64,
    /// Sats of the HTLC output: what the taker locks (submarine), or 546
    /// plus the claim's fee at `claimFeeRate` (reverse).
    pub htlc_sat: u64,
    /// **Reverse only**: the fee rate, sat/vB, `htlcSat` funds the claim at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_fee_rate: Option<u64>,
    /// The HTLC output script (hex): the same P2TR as the swap's address.
    pub script_pubkey: String,
    /// RGB proxy the lock consignment is posted to.
    pub transport_endpoints: Vec<String>,
    /// Confirmations the lock needs before the other side acts on it.
    pub min_confirmations: u8,
}

/// What the caller requires of an [`RgbLock`] beyond its agreement with the
/// swap tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbLockExpectations {
    /// The contract the caller means to swap. `None` accepts any `rgb:` id;
    /// set it whenever the caller knows the contract (a wallet does), since
    /// nothing else ties the maker's lock to the asset the caller priced.
    pub asset_id: Option<String>,
    /// The most sats a submarine may ask the taker to lock (see
    /// [`DEFAULT_MAX_SUBMARINE_HTLC_SAT`]). Not used for a reverse swap,
    /// whose HTLC sats the maker funds.
    pub max_submarine_htlc_sat: u64,
}

impl Default for RgbLockExpectations {
    fn default() -> Self {
        Self {
            asset_id: None,
            max_submarine_htlc_sat: DEFAULT_MAX_SUBMARINE_HTLC_SAT,
        }
    }
}

impl RgbLockExpectations {
    /// Expect `asset_id`, with the default limits.
    pub fn for_asset(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: Some(asset_id.into()),
            ..Self::default()
        }
    }
}

impl RgbLock {
    /// The HTLC output script.
    pub fn script_pubkey(&self) -> Result<ScriptBuf, Error> {
        ScriptBuf::from_hex(&self.script_pubkey)
            .map_err(|e| rgb_error(format!("scriptPubkey is not hex: {e}")))
    }

    /// Check the script rgb-lib decodes from [`Self::recipient_id`]
    /// (`rgb_lib::utils::script_buf_from_recipient_id`) against the HTLC.
    ///
    /// **Call it before a submarine lock.** rgb-lib's `send` pays the script
    /// the recipient id encodes, not `scriptPubkey`; this crate cannot decode
    /// the id, so without this check a recipient id for any other script
    /// would send the asset there instead of into the HTLC.
    pub fn check_recipient_script(&self, decoded: &ScriptBuf) -> Result<(), Error> {
        if *decoded != self.script_pubkey()? {
            return Err(rgb_error(
                "recipientId does not encode the HTLC script: refusing to lock",
            ));
        }
        Ok(())
    }

    /// The output of `lock_tx` that pays the HTLC, with its outpoint.
    /// Exactly one output may pay it.
    pub fn find_htlc_output(&self, lock_tx: &Transaction) -> Result<(OutPoint, TxOut), Error> {
        let script = self.script_pubkey()?;
        let txid = lock_tx.compute_txid();
        let mut found = lock_tx
            .output
            .iter()
            .enumerate()
            .filter(|(_, out)| out.script_pubkey == script);
        let (vout, txout) = found
            .next()
            .ok_or_else(|| rgb_error(format!("{txid} has no output paying the HTLC")))?;
        if found.next().is_some() {
            return Err(rgb_error(format!("{txid} pays the HTLC more than once")));
        }
        if txout.value.to_sat() < self.htlc_sat {
            return Err(rgb_error(format!(
                "the HTLC output carries {} sat, {} expected",
                txout.value.to_sat(),
                self.htlc_sat
            )));
        }
        let vout = u32::try_from(vout).map_err(|_| rgb_error("output index overflows"))?;
        Ok((OutPoint { txid, vout }, txout.clone()))
    }

    /// The checks common to both directions: the lock is on this swap's
    /// address, for the stated amount, of the expected contract, over a
    /// proxy rgb-lib can reach.
    fn validate_common(
        &self,
        lockup_address: &str,
        chain: BitcoinChain,
        swap_amount: u64,
        expected: &RgbLockExpectations,
    ) -> Result<(), Error> {
        if !self.asset_id.starts_with(RGB_ASSET_ID_PREFIX) {
            return Err(rgb_error(format!(
                "assetId {:?} is not an RGB contract id",
                self.asset_id
            )));
        }
        if let Some(want) = expected.asset_id.as_deref() {
            if self.asset_id != want {
                return Err(rgb_error(format!(
                    "the lock is for {}, not the expected {want}",
                    self.asset_id
                )));
            }
        }
        if self.amount == 0 || self.amount != swap_amount {
            return Err(rgb_error(format!(
                "the lock amount {} differs from the swap amount {swap_amount}",
                self.amount
            )));
        }
        let address = Address::from_str(lockup_address)?;
        if !address.is_valid_for_network(chain.into()) {
            return Err(rgb_error("the lockup address is for another network"));
        }
        if address.assume_checked().script_pubkey() != self.script_pubkey()? {
            return Err(rgb_error("scriptPubkey is not the swap's lockup address"));
        }
        if self.recipient_id.trim().is_empty() {
            return Err(rgb_error("recipientId is empty"));
        }
        if self.htlc_sat < MIN_HTLC_OUTPUT_SAT {
            return Err(rgb_error(format!(
                "htlcSat {} is below the {MIN_HTLC_OUTPUT_SAT} sat spend output",
                self.htlc_sat
            )));
        }
        if self.transport_endpoints.is_empty() {
            return Err(rgb_error("no transport endpoint for the lock consignment"));
        }
        if let Some(bad) = self
            .transport_endpoints
            .iter()
            .find(|e| !(e.starts_with("rpc://") || e.starts_with("rpcs://")))
        {
            return Err(rgb_error(format!(
                "transport endpoint {bad:?} is not an rgb proxy (rpc:// or rpcs://)"
            )));
        }
        if self.min_confirmations == 0 {
            return Err(rgb_error("minConfirmations must be at least 1"));
        }
        Ok(())
    }
}

impl CreateSubmarineResponse {
    /// Validate an RGB submarine create response (`USDT-RGB → BTC`) and
    /// return its lock.
    ///
    /// Runs [`Self::validate`] — the swap tree commits to the invoice's
    /// payment hash and the caller's refund key, and hashes to `address` —
    /// then checks the `rgb` object against it: the lock pays `address`, for
    /// `expectedAmount` of the expected contract, with `htlcSat` within
    /// [`RgbLockExpectations::max_submarine_htlc_sat`].
    ///
    /// Before sending, also check the recipient id with
    /// [`RgbLock::check_recipient_script`].
    pub fn validate_rgb(
        &self,
        invoice: &str,
        our_pubkey: &bitcoin::PublicKey,
        chain: BitcoinChain,
        expected: &RgbLockExpectations,
    ) -> Result<&RgbLock, Error> {
        self.validate(invoice, our_pubkey, Chain::Bitcoin(chain))?;
        self.checked_rgb_lock(chain, expected)
    }

    /// [`Self::validate_rgb`]'s checks of the `rgb` object alone.
    fn checked_rgb_lock(
        &self,
        chain: BitcoinChain,
        expected: &RgbLockExpectations,
    ) -> Result<&RgbLock, Error> {
        let lock = self
            .rgb
            .as_ref()
            .ok_or_else(|| rgb_error("the create response carries no rgb lock"))?;
        lock.validate_common(&self.address, chain, self.expected_amount, expected)?;
        if lock.claim_fee_rate.is_some() {
            return Err(rgb_error("a submarine lock carries no claimFeeRate"));
        }
        if lock.htlc_sat > expected.max_submarine_htlc_sat {
            return Err(rgb_error(format!(
                "htlcSat {} exceeds the {} sat the caller allows",
                lock.htlc_sat, expected.max_submarine_htlc_sat
            )));
        }
        Ok(lock)
    }
}

impl CreateReverseResponse {
    /// Validate an RGB reverse create response (`BTC → USDT-RGB`) and return
    /// its lock.
    ///
    /// Runs [`Self::validate`] — the invoice pays the caller's preimage hash,
    /// the swap tree commits to it and the caller's claim key, and hashes to
    /// `lockupAddress` — then checks the `rgb` object against it: the lock
    /// pays `lockupAddress`, for `onchainAmount` of the expected contract,
    /// with enough `htlcSat` to fund the claim at `claimFeeRate`, so the
    /// taker's claim needs no BTC of their own.
    pub fn validate_rgb(
        &self,
        preimage: &Preimage,
        our_pubkey: &bitcoin::PublicKey,
        chain: BitcoinChain,
        expected: &RgbLockExpectations,
    ) -> Result<&RgbLock, Error> {
        let lock = self
            .rgb
            .as_ref()
            .ok_or_else(|| rgb_error("the create response carries no rgb lock"))?;
        self.validate(preimage, our_pubkey, Chain::Bitcoin(chain))?;
        lock.validate_common(&self.lockup_address, chain, self.onchain_amount, expected)?;
        let fee_rate = lock
            .claim_fee_rate
            .filter(|rate| *rate > 0)
            .ok_or_else(|| rgb_error("a reverse lock needs a positive claimFeeRate"))?;
        let claim_leaf = ScriptBuf::from_hex(&self.swap_tree.claim_leaf.output)?;
        let needed =
            taker_claim_htlc_sat(claim_leaf_witness_weight(claim_leaf.len() as u64), fee_rate)?;
        if lock.htlc_sat < needed {
            return Err(rgb_error(format!(
                "htlcSat {} cannot fund a claim at {fee_rate} sat/vB ({needed} needed)",
                lock.htlc_sat
            )));
        }
        Ok(lock)
    }
}

/// Which leaf an [`RgbHtlcSpend`] goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbHtlcSpendKind {
    /// The taker's claim of a reverse lock, with the preimage.
    Claim,
    /// The taker's refund of its own submarine lock, after the timeout.
    Refund,
}

/// A vanilla (uncolored) BTC input of the caller's wallet that pays a
/// spend's fee, and where its change goes. The input must be a P2TR key-path
/// spend, as every rgb-lib vanilla output is: the fee is sized for its
/// witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbFeeInput {
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub change_script: ScriptBuf,
}

/// A colored claim or refund of an RGB HTLC.
///
/// 1. Build it: [`Self::claim`] or [`Self::refund`].
/// 2. Color [`Self::psbt`] with rgb-lib: `psbt_op_prepare` (or
///    `psbt_op_prepare_with_expiry`) with the asset's `output_map` set to
///    `{COLORED_OUTPUT_INDEX: amount}` and the HTLC outpoint as the input to
///    move the allocation from. rgb-lib writes the commitment into output 0.
/// 3. Sign the colored PSBT with [`Self::sign_colored`] (or
///    [`Self::sign_colored_tx`] when the HTLC is the only input). The swap
///    key signs over the colored transaction, commitment included.
/// 4. With a fee input, let the rgb-lib wallet sign and finalize its input:
///    the HTLC input's `final_script_witness` is already set.
/// 5. `psbt_op_mark_broadcast`, broadcast, `psbt_op_apply`, and settle the
///    colored receive with the operation's consignment.
#[derive(Debug, Clone)]
pub struct RgbHtlcSpend {
    kind: RgbHtlcSpendKind,
    swap_script: BtcSwapScript,
    prevouts: Vec<TxOut>,
    template: Transaction,
    fee_sat: u64,
}

fn op_return_placeholder() -> TxOut {
    TxOut {
        value: Amount::ZERO,
        script_pubkey: ScriptBuf::new_op_return([]),
    }
}

/// A P2TR script, for sizing a spend whose real destination is unknown
/// (every rgb-lib receive is P2TR).
fn p2tr_placeholder() -> ScriptBuf {
    let mut bytes = vec![0x51, 0x20];
    bytes.extend([0u8; 32]);
    ScriptBuf::from_bytes(bytes)
}

fn spend_tx(lock_time: LockTime, inputs: Vec<OutPoint>, output: Vec<TxOut>) -> Transaction {
    Transaction {
        version: Version::TWO,
        lock_time,
        input: inputs
            .into_iter()
            .map(|previous_output| TxIn {
                previous_output,
                script_sig: ScriptBuf::new(),
                sequence: HTLC_SPEND_SEQUENCE,
                witness: Witness::new(),
            })
            .collect(),
        output,
    }
}

/// The fee of `tx` once signed and committed: the segwit marker and flag,
/// `witness_weight` for every input, and the 33 bytes the commitment adds to
/// the OP_RETURN.
fn spend_fee(tx: &Transaction, witness_weight: u64, fee_rate_sat_vb: u64) -> Result<u64, Error> {
    if fee_rate_sat_vb == 0 {
        return Err(rgb_error("the spend fee rate must be positive"));
    }
    let weight = tx.weight()
        + Weight::from_wu(2 + witness_weight)
        + Weight::from_non_witness_data_size(COMMITMENT_BYTES);
    weight
        .to_vbytes_ceil()
        .checked_mul(fee_rate_sat_vb)
        .ok_or_else(|| rgb_error("the spend fee overflows"))
}

/// A written commitment: `OP_RETURN OP_PUSHBYTES_32 <32 bytes>`.
fn is_written_commitment(script: &ScriptBuf) -> bool {
    let bytes = script.as_bytes();
    bytes.len() == 34 && script.is_op_return() && bytes[1] == 0x20
}

impl RgbHtlcSpend {
    /// The taker's claim of a reverse lock: the HTLC alone, its sats paying
    /// the fee and the rest going to `dest_script` with the asset.
    ///
    /// `htlc` is the lock output ([`RgbLock::find_htlc_output`] on the lock
    /// transaction), `dest_script` a colored receive of the caller's rgb-lib
    /// wallet (`witness_receive`, decoded with
    /// `script_buf_from_recipient_id`). `fee_rate_sat_vb` defaults to the
    /// lock's `claimFeeRate`, which its sats were funded for; a higher rate
    /// may leave less than [`MIN_HTLC_OUTPUT_SAT`] and is then refused.
    ///
    /// Claim before `timeoutBlockHeight`: from then on the maker can refund.
    pub fn claim(
        swap_script: &BtcSwapScript,
        lock: &RgbLock,
        htlc: (OutPoint, TxOut),
        dest_script: ScriptBuf,
        fee_rate_sat_vb: Option<u64>,
    ) -> Result<Self, Error> {
        if swap_script.swap_type != SwapType::ReverseSubmarine {
            return Err(rgb_error(
                "only a reverse swap's lock is the taker's to claim",
            ));
        }
        let fee_rate = fee_rate_sat_vb
            .or(lock.claim_fee_rate)
            .ok_or_else(|| rgb_error("no fee rate for the claim"))?;
        Self::check_htlc(swap_script, lock, &htlc.1)?;
        let witness_weight = claim_leaf_witness_weight(swap_script.claim_script().len() as u64);
        Self::build_htlc_only(
            RgbHtlcSpendKind::Claim,
            swap_script,
            LockTime::ZERO,
            htlc,
            dest_script,
            witness_weight,
            fee_rate,
        )
    }

    /// The taker's refund of its own submarine lock through the refund leaf,
    /// valid from the block after `timeoutBlockHeight`.
    ///
    /// Without `fee_input` the HTLC pays its own fee, which a small lock
    /// often cannot at a useful rate (1 000 sat leaves less than
    /// [`MIN_HTLC_OUTPUT_SAT`] above about 3 sat/vB): the caller's wallet then
    /// adds a vanilla input, and every HTLC sat goes to `dest_script` with the
    /// asset while the fee input's remainder is change (dropped below
    /// [`MIN_HTLC_OUTPUT_SAT`]).
    pub fn refund(
        swap_script: &BtcSwapScript,
        lock: &RgbLock,
        htlc: (OutPoint, TxOut),
        dest_script: ScriptBuf,
        fee_input: Option<RgbFeeInput>,
        fee_rate_sat_vb: u64,
    ) -> Result<Self, Error> {
        if swap_script.swap_type != SwapType::Submarine {
            return Err(rgb_error("only a submarine lock is the taker's to refund"));
        }
        Self::check_htlc(swap_script, lock, &htlc.1)?;
        let witness_weight = refund_leaf_witness_weight(swap_script.refund_script().len() as u64);
        let lock_time = swap_script.locktime;
        let Some(fee_input) = fee_input else {
            return Self::build_htlc_only(
                RgbHtlcSpendKind::Refund,
                swap_script,
                lock_time,
                htlc,
                dest_script,
                witness_weight,
                fee_rate_sat_vb,
            );
        };
        if fee_input.outpoint == htlc.0 {
            return Err(rgb_error("the fee input is the HTLC itself"));
        }
        let (htlc_outpoint, htlc_txout) = htlc;
        let mut tx = spend_tx(
            lock_time,
            vec![htlc_outpoint, fee_input.outpoint],
            vec![
                op_return_placeholder(),
                TxOut {
                    value: htlc_txout.value,
                    script_pubkey: dest_script,
                },
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: fee_input.change_script.clone(),
                },
            ],
        );
        let witnesses = witness_weight + KEY_PATH_WITNESS_WEIGHT;
        let available = fee_input.txout.value.to_sat();
        let with_change = spend_fee(&tx, witnesses, fee_rate_sat_vb)?;
        let fee = match available.checked_sub(with_change) {
            Some(change) if change >= MIN_HTLC_OUTPUT_SAT => {
                tx.output[2].value = Amount::from_sat(change);
                with_change
            }
            _ => {
                tx.output.pop();
                let without_change = spend_fee(&tx, witnesses, fee_rate_sat_vb)?;
                if available < without_change {
                    return Err(rgb_error(format!(
                        "the fee input carries {available} sat, the refund needs {without_change}"
                    )));
                }
                // No change output: the whole input is fee.
                available
            }
        };
        Ok(Self {
            kind: RgbHtlcSpendKind::Refund,
            swap_script: swap_script.clone(),
            prevouts: vec![htlc_txout, fee_input.txout],
            template: tx,
            fee_sat: fee,
        })
    }

    /// [`Self::claim`] from the reverse create response the caller
    /// validated with [`CreateReverseResponse::validate_rgb`]: the swap
    /// script is rebuilt from it (checked against `lockupAddress` on
    /// `chain`) and the HTLC output found in `lock_tx`, the maker's lock
    /// transaction.
    pub fn claim_from_response(
        response: &CreateReverseResponse,
        our_pubkey: &bitcoin::PublicKey,
        chain: BitcoinChain,
        lock_tx: &Transaction,
        dest_script: ScriptBuf,
        fee_rate_sat_vb: Option<u64>,
    ) -> Result<Self, Error> {
        let lock = response
            .rgb
            .as_ref()
            .ok_or_else(|| rgb_error("the create response carries no rgb lock"))?;
        let swap_script = BtcSwapScript::reverse_from_swap_resp(response, *our_pubkey)?;
        swap_script.validate_address(chain, response.lockup_address.clone())?;
        let htlc = lock.find_htlc_output(lock_tx)?;
        Self::claim(&swap_script, lock, htlc, dest_script, fee_rate_sat_vb)
    }

    /// [`Self::refund`] from the submarine create response the caller
    /// validated with [`CreateSubmarineResponse::validate_rgb`]: the swap
    /// script is rebuilt from it (checked against `address` on `chain`) and
    /// the HTLC output found in `lock_tx`, the taker's own lock transaction.
    pub fn refund_from_response(
        response: &CreateSubmarineResponse,
        our_pubkey: &bitcoin::PublicKey,
        chain: BitcoinChain,
        lock_tx: &Transaction,
        dest_script: ScriptBuf,
        fee_input: Option<RgbFeeInput>,
        fee_rate_sat_vb: u64,
    ) -> Result<Self, Error> {
        let lock = response
            .rgb
            .as_ref()
            .ok_or_else(|| rgb_error("the create response carries no rgb lock"))?;
        let swap_script = BtcSwapScript::submarine_from_swap_resp(response, *our_pubkey)?;
        swap_script.validate_address(chain, response.address.clone())?;
        let htlc = lock.find_htlc_output(lock_tx)?;
        Self::refund(
            &swap_script,
            lock,
            htlc,
            dest_script,
            fee_input,
            fee_rate_sat_vb,
        )
    }

    fn check_htlc(swap_script: &BtcSwapScript, lock: &RgbLock, htlc: &TxOut) -> Result<(), Error> {
        let script = lock.script_pubkey()?;
        if htlc.script_pubkey != script {
            return Err(rgb_error("the HTLC output does not pay the lock's script"));
        }
        // The lock's script must be this swap tree's, or the leaf signed
        // below would not spend it.
        let tree_output_key = swap_script.taproot_spendinfo()?.output_key();
        if Address::p2tr_tweaked(tree_output_key, bitcoin::Network::Bitcoin).script_pubkey()
            != script
        {
            return Err(rgb_error("the lock's script is not this swap tree's"));
        }
        Ok(())
    }

    fn build_htlc_only(
        kind: RgbHtlcSpendKind,
        swap_script: &BtcSwapScript,
        lock_time: LockTime,
        (htlc_outpoint, htlc_txout): (OutPoint, TxOut),
        dest_script: ScriptBuf,
        witness_weight: u64,
        fee_rate_sat_vb: u64,
    ) -> Result<Self, Error> {
        let mut tx = spend_tx(
            lock_time,
            vec![htlc_outpoint],
            vec![
                op_return_placeholder(),
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: dest_script,
                },
            ],
        );
        let fee = spend_fee(&tx, witness_weight, fee_rate_sat_vb)?;
        let htlc_value = htlc_txout.value.to_sat();
        let dest = htlc_value
            .checked_sub(fee)
            .filter(|dest| *dest >= MIN_HTLC_OUTPUT_SAT)
            .ok_or_else(|| {
                rgb_error(format!(
                    "the HTLC carries {htlc_value} sat: a {fee} sat fee leaves less than \
                     {MIN_HTLC_OUTPUT_SAT} sat"
                ))
            })?;
        tx.output[1].value = Amount::from_sat(dest);
        Ok(Self {
            kind,
            swap_script: swap_script.clone(),
            prevouts: vec![htlc_txout],
            template: tx,
            fee_sat: fee,
        })
    }

    /// Claim or refund.
    pub fn kind(&self) -> RgbHtlcSpendKind {
        self.kind
    }

    /// The miner fee the spend pays, sats.
    pub fn fee_sat(&self) -> u64 {
        self.fee_sat
    }

    /// Index of the HTLC input: always the first.
    pub fn htlc_input_index(&self) -> usize {
        0
    }

    /// The unsigned transaction, with the empty OP_RETURN rgb-lib writes the
    /// commitment into.
    pub fn unsigned_tx(&self) -> &Transaction {
        &self.template
    }

    /// The unsigned PSBT for rgb-lib to color, every input's prevout set.
    pub fn psbt(&self) -> Result<Psbt, Error> {
        let mut psbt = Psbt::from_unsigned_tx(self.template.clone())
            .map_err(|e| rgb_error(format!("spend psbt: {e}")))?;
        for (input, prevout) in psbt.inputs.iter_mut().zip(&self.prevouts) {
            input.witness_utxo = Some(prevout.clone());
        }
        Ok(psbt)
    }

    /// The colored transaction must be the one built here, only output 0
    /// changed, and changed into a written commitment.
    fn check_colored(&self, colored: &Psbt) -> Result<(), Error> {
        let tx = &colored.unsigned_tx;
        let template = &self.template;
        if tx.version != template.version || tx.lock_time != template.lock_time {
            return Err(rgb_error(
                "the colored spend changed the version or locktime",
            ));
        }
        let same_inputs =
            tx.input.len() == template.input.len()
                && tx.input.iter().zip(&template.input).all(|(a, b)| {
                    a.previous_output == b.previous_output && a.sequence == b.sequence
                });
        if !same_inputs {
            return Err(rgb_error("the colored spend changed the inputs"));
        }
        if tx.output.len() != template.output.len() || tx.output[1..] != template.output[1..] {
            return Err(rgb_error("the colored spend changed the outputs"));
        }
        let commitment = &tx.output[0];
        if commitment.value != Amount::ZERO || !is_written_commitment(&commitment.script_pubkey) {
            return Err(rgb_error(
                "output 0 is not an RGB commitment: signing would burn the asset",
            ));
        }
        if let Some((index, _)) =
            colored
                .inputs
                .iter()
                .zip(&self.prevouts)
                .enumerate()
                .find(|(_, (input, prevout))| {
                    input
                        .witness_utxo
                        .as_ref()
                        .is_some_and(|utxo| utxo != *prevout)
                })
        {
            return Err(rgb_error(format!(
                "the colored spend names another prevout for input {index}"
            )));
        }
        Ok(())
    }

    /// Sign the HTLC input of the PSBT rgb-lib colored, and return it with
    /// that input finalized.
    ///
    /// Refused unless the colored transaction is [`Self::psbt`]'s with output
    /// 0 turned into a written commitment and nothing else changed, `keys`
    /// is the swap's claim key (claim) or refund key (refund), and, for a
    /// claim, `preimage` opens the hashlock.
    pub fn sign_colored(
        &self,
        colored: &Psbt,
        keys: &Keypair,
        preimage: Option<&Preimage>,
    ) -> Result<Psbt, Error> {
        self.check_colored(colored)?;
        let (leaf, own_key) = match self.kind {
            RgbHtlcSpendKind::Claim => (
                self.swap_script.claim_script(),
                self.swap_script.receiver_pubkey,
            ),
            RgbHtlcSpendKind::Refund => (
                self.swap_script.refund_script(),
                self.swap_script.sender_pubkey,
            ),
        };
        if keys.public_key() != own_key.inner {
            return Err(rgb_error("the keys are not this swap's for the spend"));
        }
        let preimage_bytes = match self.kind {
            RgbHtlcSpendKind::Claim => {
                let preimage = preimage.ok_or_else(|| rgb_error("a claim needs the preimage"))?;
                if preimage.hash160 != self.swap_script.hashlock {
                    return Err(rgb_error("the preimage does not open this swap's hashlock"));
                }
                Some(
                    preimage
                        .bytes
                        .ok_or_else(|| rgb_error("the preimage bytes are unknown"))?,
                )
            }
            RgbHtlcSpendKind::Refund => None,
        };

        let secp = Secp256k1::new();
        let leaf_hash = TapLeafHash::from_script(&leaf, LeafVersion::TapScript);
        let sighash = SighashCache::new(&colored.unsigned_tx).taproot_script_spend_signature_hash(
            self.htlc_input_index(),
            &Prevouts::All(&self.prevouts),
            leaf_hash,
            TapSighashType::Default,
        )?;
        let msg = Message::from_digest_slice(sighash.as_byte_array())?;
        let signature = Signature {
            signature: secp.sign_schnorr(&msg, keys),
            sighash_type: TapSighashType::Default,
        };
        let control_block = self
            .swap_script
            .taproot_spendinfo()?
            .control_block(&(leaf.clone(), LeafVersion::TapScript))
            .ok_or_else(|| Error::Taproot("Control block calculation failed".to_string()))?;

        let mut witness = Witness::new();
        witness.push(signature.to_vec());
        if let Some(preimage) = preimage_bytes {
            witness.push(preimage);
        }
        witness.push(leaf.as_bytes());
        witness.push(control_block.serialize());

        let mut signed = colored.clone();
        for (input, prevout) in signed.inputs.iter_mut().zip(&self.prevouts) {
            input.witness_utxo.get_or_insert_with(|| prevout.clone());
        }
        let htlc_input = &mut signed.inputs[self.htlc_input_index()];
        htlc_input.final_script_witness = Some(witness);
        htlc_input.tap_script_sigs.clear();
        htlc_input.tap_scripts.clear();
        Ok(signed)
    }

    /// [`Self::sign_colored`] for a spend whose only input is the HTLC (every
    /// claim, and a refund without a fee input): the broadcastable
    /// transaction.
    pub fn sign_colored_tx(
        &self,
        colored: &Psbt,
        keys: &Keypair,
        preimage: Option<&Preimage>,
    ) -> Result<Transaction, Error> {
        if self.template.input.len() != 1 {
            return Err(rgb_error(
                "the spend has a fee input: sign it in the wallet after sign_colored",
            ));
        }
        let signed = self.sign_colored(colored, keys, preimage)?;
        let mut tx = signed.unsigned_tx;
        tx.input[0].witness = signed.inputs[0]
            .final_script_witness
            .clone()
            .ok_or_else(|| rgb_error("the HTLC input was not finalized"))?;
        Ok(tx)
    }
}

impl BoltzApiClientV2 {
    /// Create an RGB submarine swap (`req.from` = [`USDT_RGB`]) and return
    /// the response only once [`CreateSubmarineResponse::validate_rgb`]
    /// passed.
    pub async fn create_rgb_submarine_swap(
        &self,
        req: &CreateSubmarineRequest,
        chain: BitcoinChain,
        expected: &RgbLockExpectations,
    ) -> Result<CreateSubmarineResponse, Error> {
        if req.from != USDT_RGB {
            return Err(rgb_error(format!(
                "an RGB submarine swap sends {USDT_RGB}, not {}",
                req.from
            )));
        }
        let response = self.post_swap_req(req).await?;
        response.validate_rgb(&req.invoice, &req.refund_public_key, chain, expected)?;
        Ok(response)
    }

    /// Create an RGB reverse swap (`req.to` = [`USDT_RGB`]) and return the
    /// response only once [`CreateReverseResponse::validate_rgb`] passed
    /// against the request's `preimageHash` (or its invoice's payment hash).
    pub async fn create_rgb_reverse_swap(
        &self,
        req: CreateReverseRequest,
        chain: BitcoinChain,
        expected: &RgbLockExpectations,
    ) -> Result<CreateReverseResponse, Error> {
        if req.to != USDT_RGB {
            return Err(rgb_error(format!(
                "an RGB reverse swap receives {USDT_RGB}, not {}",
                req.to
            )));
        }
        let preimage = match (&req.preimage_hash, &req.invoice) {
            (Some(hash), _) => Preimage::from_sha256_str(&hash.to_string())?,
            (None, Some(invoice)) => Preimage::from_invoice_str(invoice)?,
            (None, None) => {
                return Err(rgb_error(
                    "a reverse swap request needs preimageHash or invoice",
                ))
            }
        };
        let claim_public_key = req.claim_public_key;
        let response = self.post_reverse_req(req).await?;
        response.validate_rgb(&preimage, &claim_public_key, chain, expected)?;
        Ok(response)
    }
}

/// Which amount of an atomic quote request is fixed.
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
    /// [`BTC_TO_USDT_RGB`] or [`USDT_RGB_TO_BTC`].
    pub pair: String,
    /// Sats for BTC, the contract's smallest unit for the RGB asset.
    pub amount: u64,
    #[serde(default)]
    pub direction: AtomicAmountDirection,
}

/// The maker's price and rgb-lib offer for an atomic swap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicQuoteResponse {
    pub id: String,
    pub pair: String,
    pub direction: AtomicAmountDirection,
    /// What the taker sends.
    pub from_amount: u64,
    /// What the taker receives, net of the maker's fee.
    pub to_amount: u64,
    /// The RGB contract of the RGB leg.
    pub asset_id: String,
    /// Miner fee the taker funds on top of `fromAmount`, sats.
    pub network_fee_sat: u64,
    /// The maker's fee, in the output asset's units.
    pub service_fee: u64,
    /// Unix seconds: the request must arrive before this.
    pub expires_at: i64,
    /// Unix seconds: the completion must arrive before this.
    pub offer_expires_at: i64,
    /// rgb-lib's offer, for the taker's `accept_swap_offer`.
    pub offer: Value,
}

/// `POST /v2/swap/atomic/{id}/request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicRequestBody {
    /// rgb-lib's `OnchainSwapRequest`, from `accept_swap_offer`.
    pub request: Value,
}

/// The maker's unsigned proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicProposalResponse {
    pub id: String,
    pub status: String,
    /// rgb-lib's `OnchainSwapProposal`, for the taker's
    /// `complete_swap_proposal`.
    pub proposal: Value,
}

/// `POST /v2/swap/atomic/{id}/complete`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicCompleteBody {
    /// rgb-lib's `OnchainSwapCompletion`, from `complete_swap_proposal`.
    pub completion: Value,
}

/// The broadcast swap: the maker signs last and broadcasts in the same call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicCompleteResponse {
    pub id: String,
    pub status: String,
    pub txid: String,
    /// The finalized completion, for the taker's `accept_swap_transfers`.
    pub completion: Value,
}

/// `GET /v2/swap/atomic/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicSwapStatus {
    pub id: String,
    pub pair: String,
    /// Wire status (`swap.created` … `transaction.claimed`).
    pub status: String,
    /// Internal state (`quoted` … `settled`).
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

/// Min / max amount of an atomic pair, in the input asset's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPairLimits {
    pub minimal: u64,
    pub maximal: u64,
}

/// Fees of an atomic pair. The miner fee is quoted per swap
/// ([`AtomicQuoteResponse::network_fee_sat`]), not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPairFees {
    pub percentage: f64,
    pub miner_fees: u64,
}

/// An atomic rate card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPair {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair_id: Option<String>,
    pub hash: String,
    /// Output units per input unit, each asset in its smallest unit.
    pub rate: f64,
    pub limits: AtomicPairLimits,
    pub fees: AtomicPairFees,
}

/// `GET /v2/swap/atomic/pairs`: `from` currency → `to` currency → card.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GetAtomicPairsResponse {
    pub pairs: std::collections::HashMap<String, std::collections::HashMap<String, AtomicPair>>,
}

impl GetAtomicPairsResponse {
    /// The card for one `from`/`to` pair, if the maker serves it.
    pub fn get(&self, from: &str, to: &str) -> Option<&AtomicPair> {
        self.pairs.get(from)?.get(to)
    }
}

/// rgb-lib's swap id inside any swap message: an offer carries it at the
/// top, each later message nests the previous one.
pub fn rgb_swap_message_id(message: &Value) -> Option<&str> {
    let mut node = message;
    for _ in 0..4 {
        if let Some(id) = node.get("swap_id").and_then(Value::as_str) {
            return Some(id);
        }
        node = ["offer", "request", "proposal"]
            .iter()
            .find_map(|key| node.get(*key))?;
    }
    None
}

fn same_swap_id(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl AtomicQuoteResponse {
    /// rgb-lib's id for this swap, inside [`Self::offer`].
    pub fn rgb_swap_id(&self) -> Option<&str> {
        rgb_swap_message_id(&self.offer)
    }

    /// Check the quote answers `request`: the same pair and direction, the
    /// fixed amount unchanged, both amounts positive, an RGB contract (the
    /// expected one, when given), an offer rgb-lib can accept, and still open
    /// at `now_unix`.
    ///
    /// rgb-lib's `accept_swap_offer` re-checks the offer's legs against the
    /// amounts the caller passes it: pass it this quote's.
    pub fn validate(
        &self,
        request: &AtomicQuoteRequest,
        expected_asset_id: Option<&str>,
        now_unix: i64,
    ) -> Result<(), Error> {
        if self.pair != request.pair || self.direction != request.direction {
            return Err(rgb_error(format!(
                "the quote is for {} {:?}, not {} {:?}",
                self.pair, self.direction, request.pair, request.direction
            )));
        }
        let fixed = match request.direction {
            AtomicAmountDirection::From => self.from_amount,
            AtomicAmountDirection::To => self.to_amount,
        };
        if fixed != request.amount {
            return Err(rgb_error(format!(
                "the quote fixes {fixed}, not the requested {}",
                request.amount
            )));
        }
        if self.from_amount == 0 || self.to_amount == 0 {
            return Err(rgb_error("the quote has a zero amount"));
        }
        if !self.asset_id.starts_with(RGB_ASSET_ID_PREFIX) {
            return Err(rgb_error(format!(
                "assetId {:?} is not an RGB contract id",
                self.asset_id
            )));
        }
        if let Some(want) = expected_asset_id {
            if self.asset_id != want {
                return Err(rgb_error(format!(
                    "the quote is for {}, not the expected {want}",
                    self.asset_id
                )));
            }
        }
        if self.rgb_swap_id().is_none() {
            return Err(rgb_error("the offer carries no rgb-lib swap id"));
        }
        if self.expires_at <= now_unix {
            return Err(rgb_error("the quote has expired"));
        }
        if self.offer_expires_at < self.expires_at {
            return Err(rgb_error("the offer expires before the quote"));
        }
        Ok(())
    }
}

impl AtomicProposalResponse {
    /// Check the proposal belongs to `quote`: the maker's id and rgb-lib's.
    pub fn validate(&self, quote: &AtomicQuoteResponse) -> Result<(), Error> {
        if !same_swap_id(&self.id, &quote.id) {
            return Err(rgb_error(format!(
                "proposal for swap {}, not {}",
                self.id, quote.id
            )));
        }
        match (rgb_swap_message_id(&self.proposal), quote.rgb_swap_id()) {
            (Some(got), Some(want)) if got == want => Ok(()),
            _ => Err(rgb_error("the proposal is for another rgb-lib swap")),
        }
    }
}

impl AtomicCompleteResponse {
    /// Check the broadcast swap belongs to `quote` and names a txid.
    pub fn validate(&self, quote: &AtomicQuoteResponse) -> Result<(), Error> {
        if !same_swap_id(&self.id, &quote.id) {
            return Err(rgb_error(format!(
                "completion for swap {}, not {}",
                self.id, quote.id
            )));
        }
        bitcoin::Txid::from_str(&self.txid)
            .map_err(|e| rgb_error(format!("txid {:?}: {e}", self.txid)))?;
        match (rgb_swap_message_id(&self.completion), quote.rgb_swap_id()) {
            (Some(got), Some(want)) if got == want => Ok(()),
            _ => Err(rgb_error("the completion is for another rgb-lib swap")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swaps::boltz::{Leaf, SwapTree};
    use bitcoin::secp256k1::SecretKey;
    use bitcoin::PublicKey;
    use serde_json::json;

    const CHAIN: BitcoinChain = BitcoinChain::BitcoinRegtest;
    const ASSET: &str = "rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd";

    fn keypair(byte: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(&[byte; 32]).unwrap(),
        )
    }

    fn pubkey(keys: &Keypair) -> PublicKey {
        PublicKey::new(keys.public_key())
    }

    fn preimage() -> Preimage {
        Preimage::from_vec(vec![7u8; 32]).unwrap()
    }

    /// A reverse swap tree the taker (`taker`) claims and the maker refunds,
    /// and the create response a maker would send for it.
    fn reverse(taker: &Keypair, maker: &Keypair, htlc_sat: u64) -> CreateReverseResponse {
        let preimage = preimage();
        let script = BtcSwapScript {
            swap_type: SwapType::ReverseSubmarine,
            side: None,
            funding_addrs: None,
            hashlock: preimage.hash160,
            receiver_pubkey: pubkey(taker),
            locktime: LockTime::from_height(500).unwrap(),
            sender_pubkey: pubkey(maker),
            expected_amount: 1_000_000,
        };
        let address = script.to_address(CHAIN).unwrap();
        CreateReverseResponse {
            id: "SWAP".into(),
            invoice: None,
            swap_tree: SwapTree {
                claim_leaf: Leaf {
                    output: script.claim_script().to_hex_string(),
                    version: 192,
                },
                refund_leaf: Leaf {
                    output: script.refund_script().to_hex_string(),
                    version: 192,
                },
            },
            lockup_address: address.to_string(),
            refund_public_key: pubkey(maker),
            timeout_block_height: 500,
            onchain_amount: 1_000_000,
            blinding_key: None,
            asset_id: None,
            fee_asset_id: None,
            swap_auth: None,
            rgb: Some(RgbLock {
                asset_id: ASSET.into(),
                amount: 1_000_000,
                recipient_id: "bcrt:wvout:htlc".into(),
                blinding: 42,
                htlc_sat,
                claim_fee_rate: Some(5),
                script_pubkey: address.script_pubkey().to_hex_string(),
                transport_endpoints: vec!["rpcs://proxy.example/json-rpc".into()],
                min_confirmations: 1,
            }),
        }
    }

    fn funded_htlc_sat() -> u64 {
        let script_len = reverse(&keypair(1), &keypair(2), 0)
            .swap_tree
            .claim_leaf
            .output
            .len() as u64
            / 2;
        taker_claim_htlc_sat(claim_leaf_witness_weight(script_len), 5).unwrap()
    }

    fn submarine(taker: &Keypair, maker: &Keypair) -> (CreateSubmarineResponse, BtcSwapScript) {
        let script = BtcSwapScript {
            swap_type: SwapType::Submarine,
            side: None,
            funding_addrs: None,
            hashlock: preimage().hash160,
            receiver_pubkey: pubkey(maker),
            locktime: LockTime::from_height(600).unwrap(),
            sender_pubkey: pubkey(taker),
            expected_amount: 2_000_000,
        };
        let address = script.to_address(CHAIN).unwrap();
        let response = CreateSubmarineResponse {
            accept_zero_conf: false,
            address: address.to_string(),
            bip21: String::new(),
            claim_public_key: pubkey(maker),
            expected_amount: 2_000_000,
            id: "SUB".into(),
            referral_id: None,
            swap_tree: SwapTree {
                claim_leaf: Leaf {
                    output: script.claim_script().to_hex_string(),
                    version: 192,
                },
                refund_leaf: Leaf {
                    output: script.refund_script().to_hex_string(),
                    version: 192,
                },
            },
            timeout_block_height: 600,
            blinding_key: None,
            asset_id: None,
            fee_asset_id: None,
            swap_auth: None,
            rgb: Some(RgbLock {
                asset_id: ASSET.into(),
                amount: 2_000_000,
                recipient_id: "bcrt:wvout:htlc".into(),
                blinding: 43,
                htlc_sat: 1_000,
                claim_fee_rate: None,
                script_pubkey: address.script_pubkey().to_hex_string(),
                transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".into()],
                min_confirmations: 1,
            }),
        };
        (response, script)
    }

    fn lock_output(lock: &RgbLock, value: u64) -> (OutPoint, TxOut) {
        (
            OutPoint::from_str(
                "1111111111111111111111111111111111111111111111111111111111111111:1",
            )
            .unwrap(),
            TxOut {
                value: Amount::from_sat(value),
                script_pubkey: lock.script_pubkey().unwrap(),
            },
        )
    }

    fn dest() -> ScriptBuf {
        let mut bytes = vec![0x51, 0x20];
        bytes.extend([3u8; 32]);
        ScriptBuf::from_bytes(bytes)
    }

    /// What rgb-lib's `psbt_op_prepare` does to output 0.
    fn color(mut psbt: Psbt) -> Psbt {
        psbt.unsigned_tx.output[0].script_pubkey = ScriptBuf::new_op_return([9u8; 32]);
        psbt
    }

    #[test]
    fn witness_weights_and_claim_sats_match_the_maker() {
        // The maker's own pins (maker-layer-rgb htlc.rs).
        assert_eq!(claim_leaf_witness_weight(61), 1 + 65 + 33 + 62 + 66);
        assert_eq!(refund_leaf_witness_weight(37), 1 + 65 + 38 + 66);
        let weight = claim_leaf_witness_weight(61);
        let fee = taker_claim_htlc_sat(weight, 5).unwrap() - MIN_HTLC_OUTPUT_SAT;
        assert!((950..1_000).contains(&fee), "fee {fee}");
        assert!(taker_claim_htlc_sat(weight, 0).is_err());
    }

    #[test]
    fn the_rgb_lock_round_trips_the_maker_json() {
        let wire = json!({
            "assetId": ASSET,
            "amount": 1_000_000,
            "recipientId": "bcrt:wvout:abc",
            "blinding": 42,
            "htlcSat": 1_000,
            "scriptPubkey": "5120",
            "transportEndpoints": ["rpcs://proxy/json-rpc"],
            "minConfirmations": 1
        });
        let lock: RgbLock = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(lock.claim_fee_rate, None);
        assert_eq!(serde_json::to_value(&lock).unwrap(), wire);
    }

    #[test]
    fn a_reverse_lock_validates_and_each_tampering_is_refused() {
        let (taker, maker) = (keypair(1), keypair(2));
        let expected = RgbLockExpectations::for_asset(ASSET);
        let ok = reverse(&taker, &maker, funded_htlc_sat());
        ok.validate_rgb(&preimage(), &pubkey(&taker), CHAIN, &expected)
            .unwrap();

        let refused = |mutate: &dyn Fn(&mut CreateReverseResponse), why: &str| {
            let mut response = ok.clone();
            mutate(&mut response);
            let err = response
                .validate_rgb(&preimage(), &pubkey(&taker), CHAIN, &expected)
                .unwrap_err()
                .to_string();
            assert!(err.contains(why), "{why}: {err}");
        };
        refused(&|r| r.rgb = None, "no rgb lock");
        refused(
            &|r| r.rgb.as_mut().unwrap().asset_id = "rgb:other".into(),
            "not the expected",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().asset_id = "USDT".into(),
            "not an RGB contract",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().amount = 999_999,
            "differs from the swap amount",
        );
        refused(
            &|r| {
                r.rgb.as_mut().unwrap().script_pubkey = dest().to_hex_string();
            },
            "not the swap's lockup address",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().htlc_sat -= 1,
            "cannot fund a claim",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().claim_fee_rate = None,
            "claimFeeRate",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().transport_endpoints = vec!["https://x".into()],
            "not an rgb proxy",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().transport_endpoints.clear(),
            "no transport",
        );
        refused(
            &|r| r.rgb.as_mut().unwrap().min_confirmations = 0,
            "minConfirmations",
        );
        // The swap tree check still runs: a lock for another taker is refused.
        let other = reverse(&keypair(3), &maker, funded_htlc_sat());
        assert!(other
            .validate_rgb(&preimage(), &pubkey(&taker), CHAIN, &expected)
            .is_err());
    }

    #[test]
    fn a_submarine_lock_validates_and_caps_the_htlc_sats() {
        let (taker, maker) = (keypair(1), keypair(2));
        let (ok, _) = submarine(&taker, &maker);
        let expected = RgbLockExpectations::for_asset(ASSET);
        ok.checked_rgb_lock(CHAIN, &expected).unwrap();
        assert!(ok
            .checked_rgb_lock(BitcoinChain::Bitcoin, &expected)
            .is_err());

        let refused = |mutate: &dyn Fn(&mut RgbLock), why: &str| {
            let mut response = ok.clone();
            mutate(response.rgb.as_mut().unwrap());
            let err = response
                .checked_rgb_lock(CHAIN, &expected)
                .unwrap_err()
                .to_string();
            assert!(err.contains(why), "{why}: {err}");
        };
        refused(
            &|l| l.htlc_sat = DEFAULT_MAX_SUBMARINE_HTLC_SAT + 1,
            "the caller allows",
        );
        refused(&|l| l.htlc_sat = MIN_HTLC_OUTPUT_SAT - 1, "below");
        refused(&|l| l.claim_fee_rate = Some(5), "no claimFeeRate");
        refused(&|l| l.amount = 1, "differs from the swap amount");
        refused(&|l| l.recipient_id = " ".into(), "recipientId");

        // The full check runs the swap tree's first: an invoice for another
        // payment hash is refused before the lock is looked at.
        assert!(ok
            .validate_rgb("lnbcrt1", &pubkey(&taker), CHAIN, &expected)
            .is_err());
    }

    #[test]
    fn the_recipient_script_must_be_the_htlc() {
        let (response, _) = submarine(&keypair(1), &keypair(2));
        let lock = response.rgb.unwrap();
        lock.check_recipient_script(&lock.script_pubkey().unwrap())
            .unwrap();
        assert!(lock.check_recipient_script(&dest()).is_err());
    }

    #[test]
    fn the_htlc_output_is_found_in_the_lock_transaction() {
        let (response, _) = submarine(&keypair(1), &keypair(2));
        let lock = response.rgb.unwrap();
        let htlc = TxOut {
            value: Amount::from_sat(1_000),
            script_pubkey: lock.script_pubkey().unwrap(),
        };
        let other = TxOut {
            value: Amount::from_sat(5_000),
            script_pubkey: dest(),
        };
        let tx = spend_tx(
            LockTime::ZERO,
            vec![OutPoint::null()],
            vec![op_return_placeholder(), other.clone(), htlc.clone()],
        );
        let (outpoint, txout) = lock.find_htlc_output(&tx).unwrap();
        assert_eq!(outpoint.vout, 2);
        assert_eq!(txout, htlc);

        let twice = spend_tx(LockTime::ZERO, vec![], vec![htlc.clone(), htlc]);
        assert!(lock.find_htlc_output(&twice).is_err());
        let none = spend_tx(LockTime::ZERO, vec![], vec![other]);
        assert!(lock.find_htlc_output(&none).is_err());
    }

    #[test]
    fn the_taker_claims_a_reverse_lock_without_btc_of_their_own() {
        let (taker, maker) = (keypair(1), keypair(2));
        let response = reverse(&taker, &maker, funded_htlc_sat());
        let lock = response.rgb.clone().unwrap();
        let swap_script = BtcSwapScript::reverse_from_swap_resp(&response, pubkey(&taker)).unwrap();
        let htlc = lock_output(&lock, lock.htlc_sat);
        let spend = RgbHtlcSpend::claim(&swap_script, &lock, htlc.clone(), dest(), None).unwrap();

        let psbt = spend.psbt().unwrap();
        let tx = &psbt.unsigned_tx;
        assert_eq!(tx.input.len(), 1, "no BTC of the taker's own");
        assert_eq!(tx.input[0].previous_output, htlc.0);
        assert_eq!(tx.input[0].sequence, HTLC_SPEND_SEQUENCE);
        assert_eq!(tx.lock_time, LockTime::ZERO);
        assert!(tx.output[0].script_pubkey.is_op_return());
        assert_eq!(tx.output[1].script_pubkey, dest());
        assert_eq!(tx.output[1].value.to_sat(), MIN_HTLC_OUTPUT_SAT);
        assert_eq!(spend.fee_sat() + MIN_HTLC_OUTPUT_SAT, lock.htlc_sat);
        assert_eq!(psbt.inputs[0].witness_utxo.as_ref(), Some(&htlc.1));

        // Uncolored: refused, it would burn the asset.
        let err = spend
            .sign_colored(&psbt, &taker, Some(&preimage()))
            .unwrap_err();
        assert!(err.to_string().contains("burn"), "{err}");

        let colored = color(psbt);
        let signed = spend
            .sign_colored_tx(&colored, &taker, Some(&preimage()))
            .unwrap();
        let witness = &signed.input[0].witness;
        assert_eq!(
            witness.len(),
            4,
            "[sig, preimage, claim leaf, control block]"
        );
        assert_eq!(witness.nth(1).unwrap(), [7u8; 32]);
        assert_eq!(
            witness.nth(2).unwrap(),
            swap_script.claim_script().as_bytes()
        );
        // The quoted rate is paid by the signed, committed transaction.
        assert!(spend.fee_sat() >= signed.vsize() as u64 * 5);

        // The signature verifies over the colored transaction.
        let leaf_hash =
            TapLeafHash::from_script(&swap_script.claim_script(), LeafVersion::TapScript);
        let sighash = SighashCache::new(&signed)
            .taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&[&htlc.1]),
                leaf_hash,
                TapSighashType::Default,
            )
            .unwrap();
        let sig =
            bitcoin::secp256k1::schnorr::Signature::from_slice(witness.nth(0).unwrap()).unwrap();
        Secp256k1::new()
            .verify_schnorr(
                &sig,
                &Message::from_digest_slice(sighash.as_byte_array()).unwrap(),
                &taker.x_only_public_key().0,
            )
            .unwrap();
    }

    #[test]
    fn a_claim_refuses_the_wrong_key_preimage_or_a_rewritten_transaction() {
        let (taker, maker) = (keypair(1), keypair(2));
        let response = reverse(&taker, &maker, funded_htlc_sat());
        let lock = response.rgb.clone().unwrap();
        let swap_script = BtcSwapScript::reverse_from_swap_resp(&response, pubkey(&taker)).unwrap();
        let spend = RgbHtlcSpend::claim(
            &swap_script,
            &lock,
            lock_output(&lock, lock.htlc_sat),
            dest(),
            None,
        )
        .unwrap();
        let colored = color(spend.psbt().unwrap());

        assert!(spend
            .sign_colored(&colored, &maker, Some(&preimage()))
            .is_err());
        let wrong = Preimage::from_vec(vec![8u8; 32]).unwrap();
        assert!(spend.sign_colored(&colored, &taker, Some(&wrong)).is_err());
        assert!(spend.sign_colored(&colored, &taker, None).is_err());

        let mut redirected = colored.clone();
        redirected.unsigned_tx.output[1].script_pubkey = p2tr_placeholder();
        assert!(spend
            .sign_colored(&redirected, &taker, Some(&preimage()))
            .is_err());
        let mut extra = colored.clone();
        extra.unsigned_tx.output.push(TxOut {
            value: Amount::from_sat(1),
            script_pubkey: p2tr_placeholder(),
        });
        extra.outputs.push(Default::default());
        assert!(spend
            .sign_colored(&extra, &taker, Some(&preimage()))
            .is_err());

        // A higher rate than the lock funded leaves dust: refused.
        assert!(RgbHtlcSpend::claim(
            &swap_script,
            &lock,
            lock_output(&lock, lock.htlc_sat),
            dest(),
            Some(6)
        )
        .is_err());
        // A lock output that is not this swap's.
        let mut stranger = lock.clone();
        stranger.script_pubkey = dest().to_hex_string();
        assert!(RgbHtlcSpend::claim(
            &swap_script,
            &stranger,
            (
                OutPoint::null(),
                TxOut {
                    value: Amount::from_sat(lock.htlc_sat),
                    script_pubkey: dest()
                }
            ),
            dest(),
            None
        )
        .is_err());
    }

    #[test]
    fn the_taker_refunds_a_submarine_lock_after_the_timeout() {
        let (taker, maker) = (keypair(1), keypair(2));
        let (response, swap_script) = submarine(&taker, &maker);
        let lock = response.rgb.clone().unwrap();
        let htlc = lock_output(&lock, 1_000);

        // 1 000 sat cannot pay its own refund at 10 sat/vB.
        assert!(RgbHtlcSpend::refund(&swap_script, &lock, htlc.clone(), dest(), None, 10).is_err());
        // It can at 1 sat/vB.
        let alone =
            RgbHtlcSpend::refund(&swap_script, &lock, htlc.clone(), dest(), None, 1).unwrap();
        let tx = alone
            .sign_colored_tx(&color(alone.psbt().unwrap()), &taker, None)
            .unwrap();
        assert_eq!(tx.lock_time, LockTime::from_height(600).unwrap());
        assert_eq!(
            tx.input[0].witness.len(),
            3,
            "[sig, refund leaf, control block]"
        );

        // With a fee input: every HTLC sat to the colored output, change back.
        let fee_input = RgbFeeInput {
            outpoint: OutPoint::from_str(
                "2222222222222222222222222222222222222222222222222222222222222222:0",
            )
            .unwrap(),
            txout: TxOut {
                value: Amount::from_sat(20_000),
                script_pubkey: p2tr_placeholder(),
            },
            change_script: p2tr_placeholder(),
        };
        let spend = RgbHtlcSpend::refund(
            &swap_script,
            &lock,
            htlc.clone(),
            dest(),
            Some(fee_input.clone()),
            10,
        )
        .unwrap();
        let unsigned = spend.unsigned_tx();
        assert_eq!(unsigned.input.len(), 2);
        assert!(unsigned
            .input
            .iter()
            .all(|i| i.sequence == HTLC_SPEND_SEQUENCE));
        assert_eq!(unsigned.output[1].value.to_sat(), 1_000);
        assert_eq!(
            unsigned.output[2].value.to_sat() + spend.fee_sat(),
            fee_input.txout.value.to_sat()
        );
        let colored = color(spend.psbt().unwrap());
        assert!(spend.sign_colored_tx(&colored, &taker, None).is_err());
        let signed = spend.sign_colored(&colored, &taker, None).unwrap();
        assert!(signed.inputs[0].final_script_witness.is_some());
        assert!(
            signed.inputs[1].final_script_witness.is_none(),
            "the wallet's to sign"
        );
        // The maker's key is not the refund key.
        assert!(spend.sign_colored(&colored, &maker, None).is_err());

        // A fee input too small for the refund.
        let short = RgbFeeInput {
            txout: TxOut {
                value: Amount::from_sat(100),
                script_pubkey: p2tr_placeholder(),
            },
            ..fee_input
        };
        assert!(RgbHtlcSpend::refund(&swap_script, &lock, htlc, dest(), Some(short), 10).is_err());
    }

    #[test]
    fn spends_are_refused_on_the_wrong_side() {
        let (taker, maker) = (keypair(1), keypair(2));
        let (response, sub_script) = submarine(&taker, &maker);
        let lock = response.rgb.unwrap();
        assert!(RgbHtlcSpend::claim(
            &sub_script,
            &lock,
            lock_output(&lock, 5_000),
            dest(),
            Some(1)
        )
        .is_err());
        let reverse = reverse(&taker, &maker, funded_htlc_sat());
        let rev_lock = reverse.rgb.clone().unwrap();
        let rev_script = BtcSwapScript::reverse_from_swap_resp(&reverse, pubkey(&taker)).unwrap();
        assert!(RgbHtlcSpend::refund(
            &rev_script,
            &rev_lock,
            lock_output(&rev_lock, 5_000),
            dest(),
            None,
            1
        )
        .is_err());
    }

    fn quote() -> AtomicQuoteResponse {
        serde_json::from_value(json!({
            "id": "01J9ATOMIC",
            "pair": BTC_TO_USDT_RGB,
            "direction": "from",
            "fromAmount": 100_000,
            "toAmount": 60_000_000,
            "assetId": ASSET,
            "networkFeeSat": 700,
            "serviceFee": 120_000,
            "expiresAt": 2_000,
            "offerExpiresAt": 2_600,
            "offer": { "swap_id": "abcdefABCDEF0123456789abcdefABCD", "maker": {} }
        }))
        .unwrap()
    }

    #[test]
    fn an_atomic_quote_must_answer_the_request() {
        let request = AtomicQuoteRequest {
            pair: BTC_TO_USDT_RGB.into(),
            amount: 100_000,
            direction: AtomicAmountDirection::From,
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({ "pair": "BTC/USDT-RGB", "amount": 100_000, "direction": "from" })
        );
        let ok = quote();
        ok.validate(&request, Some(ASSET), 1_000).unwrap();
        assert_eq!(ok.rgb_swap_id(), Some("abcdefABCDEF0123456789abcdefABCD"));

        assert!(
            ok.validate(&request, Some(ASSET), 2_000).is_err(),
            "expired"
        );
        assert!(ok.validate(&request, Some("rgb:other"), 1_000).is_err());
        let other_amount = AtomicQuoteRequest {
            amount: 99_999,
            ..request.clone()
        };
        assert!(ok.validate(&other_amount, None, 1_000).is_err());
        let to = AtomicQuoteRequest {
            direction: AtomicAmountDirection::To,
            ..request.clone()
        };
        assert!(ok.validate(&to, None, 1_000).is_err());
        let mut no_offer = ok.clone();
        no_offer.offer = json!({});
        assert!(no_offer.validate(&request, None, 1_000).is_err());
    }

    #[test]
    fn atomic_steps_must_stay_on_the_quoted_swap() {
        let quote = quote();
        let proposal: AtomicProposalResponse = serde_json::from_value(json!({
            "id": "01j9atomic",
            "status": "swap.created",
            "proposal": { "request": { "offer": { "swap_id": "abcdefABCDEF0123456789abcdefABCD" } } }
        }))
        .unwrap();
        proposal.validate(&quote).unwrap();
        let mut foreign = proposal.clone();
        foreign.proposal = json!({ "request": { "offer": { "swap_id": "other" } } });
        assert!(foreign.validate(&quote).is_err());

        let complete: AtomicCompleteResponse = serde_json::from_value(json!({
            "id": "01J9ATOMIC",
            "status": "transaction.mempool",
            "txid": "1111111111111111111111111111111111111111111111111111111111111111",
            "completion": { "proposal": { "request": { "offer": { "swap_id": "abcdefABCDEF0123456789abcdefABCD" } } } }
        }))
        .unwrap();
        complete.validate(&quote).unwrap();
        let mut bad_txid = complete.clone();
        bad_txid.txid = "nope".into();
        assert!(bad_txid.validate(&quote).is_err());
    }

    #[test]
    fn atomic_pairs_and_status_parse_the_maker_json() {
        let pairs: GetAtomicPairsResponse = serde_json::from_value(json!({
            "BTC": { "USDT-RGB": {
                "pairId": "BTC/USDT-RGB",
                "hash": "ab",
                "rate": 600.0,
                "limits": { "minimal": 10_000, "maximal": 1_000_000 },
                "fees": { "percentage": 0.2, "minerFees": 0 }
            } }
        }))
        .unwrap();
        let card = pairs.get("BTC", USDT_RGB).unwrap();
        assert_eq!(card.limits.maximal, 1_000_000);
        assert!(pairs.get(USDT_RGB, "BTC").is_none());

        let status: AtomicSwapStatus = serde_json::from_value(json!({
            "id": "01J9ATOMIC",
            "pair": "BTC/USDT-RGB",
            "status": "transaction.confirmed",
            "state": "confirmed",
            "fromAmount": 100_000,
            "toAmount": 60_000_000,
            "assetId": ASSET,
            "networkFeeSat": 700,
            "txid": null,
            "confirmations": 0,
            "expiresAt": 2_000,
            "offerExpiresAt": 2_600,
            "createdAt": 1_000
        }))
        .unwrap();
        assert_eq!(status.txid, None);
    }
}
