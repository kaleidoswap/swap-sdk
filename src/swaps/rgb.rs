//! RGB-on-L1 HTLC swaps against the KaleidoSwap maker.
//!
//! The maker's USDT-RGB routes are ordinary Boltz submarine and reverse swaps
//! whose taproot HTLC output carries an RGB allocation besides its `htlcSat`
//! sats. Two routes use it:
//!
//! - **Submarine** `USDT-RGB → BTC` (Lightning): the taker locks the asset and
//!   refunds it through the refund leaf after the timeout or the explicit
//!   colored cooperative key-path flow before it.
//! - **Reverse** `BTC` (Lightning) `→ USDT-RGB`: the maker locks the asset and
//!   the taker claims it through the claim leaf.
//!
//! RGB state never enters this SDK. The caller's RGB wallet (rgb-lib) locks,
//! accepts consignments and colors spends; the SDK validates the swap, builds
//! the spend skeleton, checks what the wallet colored, and signs the HTLC leaf or cooperative key path.
//! See `docs/rgb-swaps-plan.md` for the whole flow.
//!
//! A spend of an RGB HTLC without the RGB commitment at output 0 **burns the
//! asset**. The uncolored spend paths ([`super::bitcoin::BtcSwapTx`],
//! generic cooperative signing, [`super::SwapScript::construct_claim`] and
//! [`super::SwapScript::construct_refund`]) therefore refuse a script that
//! carries an [`RgbHtlcContext`]; [`PreparedRgbSpend`] is the only way out.

use std::collections::HashSet;
use std::str::FromStr;

use bitcoin::base64::alphabet::Alphabet;
use bitcoin::base64::engine::general_purpose::NO_PAD;
use bitcoin::base64::engine::GeneralPurpose;
use bitcoin::base64::Engine;
use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::key::TweakedPublicKey;
use bitcoin::psbt::Psbt;
use bitcoin::secp256k1::{Keypair, Message, Secp256k1};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::transaction::Version;
use bitcoin::TapSighashType;
use bitcoin::{
    Address, Amount, OutPoint, Script, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Weight,
    Witness, XOnlyPublicKey,
};
use secp256k1_musig::{musig, Scalar};
use serde::{Deserialize, Serialize};

use super::bitcoin::{
    convert_keypair, convert_public_key, convert_schnorr_signature, BtcSwapScript,
};
use super::boltz::{SwapTxKind, SwapType};
use crate::error::Error;
use crate::network::BitcoinChain;
use crate::util::secrets::rng_32b;
use crate::util::secrets::Preimage;

/// The smallest colored output the maker locks to or accepts, in sats.
pub const MIN_COLORED_OUTPUT_SAT: u64 = 546;

/// Default maximum BTC collateral the taker authorizes for a submarine RGB lock.
/// The maker receives these sats on a successful claim; this is not a miner fee.
pub const DEFAULT_MAX_SUBMARINE_HTLC_SAT: u64 = 1_000;

/// Sequence of the HTLC input of every RGB spend, claim or refund: the same
/// as the maker's own spends.
pub const RGB_SPEND_SEQUENCE: Sequence = Sequence::ENABLE_RBF_NO_LOCKTIME;

/// Output that carries the RGB commitment once the wallet colors the spend.
pub const RGB_COMMITMENT_OUTPUT_INDEX: u32 = 0;

/// Output the transition assigns the asset to: the colored payout.
pub const RGB_PAYMENT_OUTPUT_INDEX: u32 = 1;

/// Bytes the commitment adds to output 0 when the wallet colors the spend:
/// `OP_RETURN OP_0` becomes `OP_RETURN <32 bytes>`.
const COMMITMENT_BYTES: usize = 32;

/// Internal key and one sibling leaf hash.
const CONTROL_BLOCK_BYTES: u64 = 1 + 32 + 32;

/// Bound on what a wallet may add to a caller-funded spend.
const MAX_RGB_PSBT_INPUTS: usize = 64;
const MAX_RGB_PSBT_OUTPUTS: usize = 64;

/// The `rgb` object of a KaleidoSwap create response: what the HTLC carries
/// and how its consignment moves.
///
/// Every amount is in the contract's own units (6 decimals for USDT-RGB),
/// except `htlc_sat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbLock {
    /// Versioned early-refund protocol advertised by the maker; absent on older makers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooperative_refund: Option<String>,
    /// RGB contract id (`rgb:…`).
    pub asset_id: String,
    /// Asset amount locked in the HTLC. Equals `expectedAmount` (submarine)
    /// or `onchainAmount` (reverse).
    pub amount: u64,
    /// rgb-lib witness recipient id of the HTLC script, the proxy key the
    /// lock consignment is posted under.
    pub recipient_id: String,
    /// Seal blinding the maker chose, as a decimal string.
    pub blinding: String,
    /// Sats on the HTLC output: the least the taker locks (submarine), or
    /// exactly what the maker locks, which funds the taker's claim (reverse).
    pub htlc_sat: u64,
    /// Reverse only: the fee rate, in sat/vB, `htlc_sat` funds the claim at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_fee_rate: Option<u64>,
    /// The HTLC output script, hex. The same P2TR as the swap address.
    pub script_pubkey: String,
    /// RGB proxy endpoints the lock consignment is posted to.
    pub transport_endpoints: Vec<String>,
    /// Confirmations the lock needs before the other side acts on it.
    pub min_confirmations: u8,
}

/// The RGB networks a witness recipient id can name, by the prefix rgb-lib
/// writes for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbChainNet {
    BitcoinMainnet,
    BitcoinTestnet3,
    BitcoinTestnet4,
    BitcoinSignet,
    /// A custom signet, such as Mutinynet, where the KaleidoSwap signet maker runs.
    BitcoinSignetCustom,
    BitcoinRegtest,
}

impl RgbChainNet {
    pub fn prefix(self) -> &'static str {
        match self {
            RgbChainNet::BitcoinMainnet => "bc",
            RgbChainNet::BitcoinTestnet3 => "tb3",
            RgbChainNet::BitcoinTestnet4 => "tb4",
            RgbChainNet::BitcoinSignet => "sb",
            RgbChainNet::BitcoinSignetCustom => "sbc",
            RgbChainNet::BitcoinRegtest => "bcrt",
        }
    }

    pub fn from_prefix(prefix: &str) -> Option<Self> {
        [
            RgbChainNet::BitcoinMainnet,
            RgbChainNet::BitcoinTestnet3,
            RgbChainNet::BitcoinTestnet4,
            RgbChainNet::BitcoinSignet,
            RgbChainNet::BitcoinSignetCustom,
            RgbChainNet::BitcoinRegtest,
        ]
        .into_iter()
        .find(|net| net.prefix() == prefix)
    }

    /// Whether a recipient on this RGB network can belong to a swap on
    /// `chain`. The SDK has one signet and one testnet chain, so each admits
    /// both of RGB's spellings.
    pub fn is_compatible_with(self, chain: BitcoinChain) -> bool {
        matches!(
            (chain, self),
            (BitcoinChain::Bitcoin, RgbChainNet::BitcoinMainnet)
                | (
                    BitcoinChain::BitcoinTestnet,
                    RgbChainNet::BitcoinTestnet3 | RgbChainNet::BitcoinTestnet4
                )
                | (
                    BitcoinChain::BitcoinSignet,
                    RgbChainNet::BitcoinSignet | RgbChainNet::BitcoinSignetCustom
                )
                | (BitcoinChain::BitcoinRegtest, RgbChainNet::BitcoinRegtest)
        )
    }
}

const BAID64_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_~";
const WITNESS_VOUT_HRI: &str = "wvout";
const PAY2VOUT_P2TR: u8 = 5;
const PAY2VOUT_LEN: usize = 33;
const BAID64_CHECKSUM_LEN: usize = 4;
const BAID64_CHUNK_FIRST: usize = 8;
const BAID64_CHUNK_LEN: usize = 7;

fn baid64_engine() -> GeneralPurpose {
    GeneralPurpose::new(
        &Alphabet::new(BAID64_ALPHABET).expect("the Baid64 alphabet is valid"),
        NO_PAD,
    )
}

/// rust-baid64's checksum: SHA-256 keyed by SHA-256 of the HRI. It takes
/// bytes 0, 1, 1 and 2 of the digest, and that repetition is part of the
/// format.
fn baid64_checksum(hri: &str, payload: &[u8]) -> [u8; BAID64_CHECKSUM_LEN] {
    let key = sha256::Hash::hash(hri.as_bytes());
    let mut engine = sha256::Hash::engine();
    engine.input(key.as_byte_array());
    engine.input(payload);
    let digest = sha256::Hash::from_engine(engine).to_byte_array();
    [digest[0], digest[1], digest[1], digest[2]]
}

/// The rgb-lib witness recipient id of a P2TR script on `network`.
///
/// This is rgb-lib's `recipient_id_from_script_buf`: `<network>:wvout:` and
/// then the chunked Baid64 of `05 ‖ output key` with its checksum embedded.
/// It is ported rather than depended on, so the SDK stays free of rgb-lib.
pub fn rgb_recipient_id(script_pubkey: &Script, network: RgbChainNet) -> Result<String, Error> {
    if !script_pubkey.is_p2tr() {
        return Err(Error::Protocol(
            "RGB witness recipients are derived for P2TR HTLC outputs only".to_string(),
        ));
    }
    let mut payload = [0u8; PAY2VOUT_LEN];
    payload[0] = PAY2VOUT_P2TR;
    payload[1..].copy_from_slice(&script_pubkey.as_bytes()[2..34]);
    let mut data = payload.to_vec();
    data.extend(baid64_checksum(WITNESS_VOUT_HRI, &payload));
    let encoded = baid64_engine().encode(data);

    let mut id = format!(
        "{}:{WITNESS_VOUT_HRI}:{}",
        network.prefix(),
        &encoded[..BAID64_CHUNK_FIRST]
    );
    for chunk in encoded.as_bytes()[BAID64_CHUNK_FIRST..].chunks(BAID64_CHUNK_LEN) {
        id.push('-');
        id.push_str(std::str::from_utf8(chunk).expect("Baid64 is ASCII"));
    }
    Ok(id)
}

/// The network and P2TR script an rgb-lib witness recipient id names.
///
/// Accepts only the canonical spelling [`rgb_recipient_id`] produces: an
/// embedded checksum, no mnemonic suffix and no `+internal key`. Anything
/// else is refused rather than normalized, so the id the wallet sends to is
/// exactly the one checked here.
pub fn rgb_recipient_script(recipient_id: &str) -> Result<(RgbChainNet, ScriptBuf), Error> {
    let invalid = |why: &str| Error::Protocol(format!("Invalid RGB recipient id: {why}"));
    let (prefix, rest) = recipient_id
        .split_once(':')
        .ok_or_else(|| invalid("missing network prefix"))?;
    let network =
        RgbChainNet::from_prefix(prefix).ok_or_else(|| invalid("not a Bitcoin RGB network"))?;
    let body = rest
        .strip_prefix(WITNESS_VOUT_HRI)
        .and_then(|body| body.strip_prefix(':'))
        .ok_or_else(|| invalid("not a witness-output recipient"))?;
    let data = baid64_engine()
        .decode(body.replace('-', ""))
        .map_err(|_| invalid("not Baid64"))?;
    if data.len() != PAY2VOUT_LEN + BAID64_CHECKSUM_LEN {
        return Err(invalid("wrong length"));
    }
    let (payload, checksum) = data.split_at(PAY2VOUT_LEN);
    if checksum != baid64_checksum(WITNESS_VOUT_HRI, payload) {
        return Err(invalid("checksum mismatch"));
    }
    if payload[0] != PAY2VOUT_P2TR {
        return Err(invalid("not a P2TR output"));
    }
    let output_key =
        XOnlyPublicKey::from_slice(&payload[1..]).map_err(|_| invalid("invalid output key"))?;
    let script =
        ScriptBuf::new_p2tr_tweaked(TweakedPublicKey::dangerous_assume_tweaked(output_key));
    if rgb_recipient_id(&script, network)? != recipient_id {
        return Err(invalid("not in canonical form"));
    }
    Ok((network, script))
}

/// Contract ids compare without their cosmetic chunk dashes.
fn same_contract_id(a: &str, b: &str) -> bool {
    a.replace('-', "") == b.replace('-', "")
}

/// A parsed, structurally checked [`RgbLock`]: what an RGB swap's HTLC carries.
///
/// The parse alone binds the recipient id to the HTLC script. Binding the
/// script to the swap, the asset to the contract the caller expects and the
/// amount to the swap amount is
/// [`crate::swaps::boltz::CreateSubmarineResponse::validate_rgb`] /
/// [`crate::swaps::boltz::CreateReverseResponse::validate_rgb`].
///
/// # Locking (submarine)
///
/// The taker locks with rgb-lib's `send`. It uses `donation: true`, because
/// the maker never ACKs, and one recipient built from these fields:
///
/// ```text
/// Recipient {
///     recipient_id: recipient_id,
///     witness_data: Some(WitnessData { amount_sat: htlc_sat, blinding: Some(blinding) }),
///     assignment: Assignment::Fungible(amount),
///     transport_endpoints: transport_endpoints,
/// }
/// ```
///
/// Keep the lock transaction's id: a refund spends exactly that outpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbHtlcContext {
    pub cooperative_refund: Option<String>,
    pub contract_id: String,
    pub amount: u64,
    pub recipient_id: String,
    pub recipient_network: RgbChainNet,
    pub blinding: u64,
    pub htlc_sat: u64,
    pub claim_fee_rate: Option<u64>,
    pub script_pubkey: ScriptBuf,
    pub transport_endpoints: Vec<String>,
    pub min_confirmations: u8,
}

impl RgbHtlcContext {
    pub fn from_lock(lock: &RgbLock) -> Result<Self, Error> {
        let script_pubkey = ScriptBuf::from_hex(&lock.script_pubkey)
            .map_err(|_| Error::Protocol("RGB scriptPubkey is not hex".to_string()))?;
        let (recipient_network, recipient_script) = rgb_recipient_script(&lock.recipient_id)?;
        if recipient_script != script_pubkey {
            return Err(Error::Protocol(
                "RGB recipient id does not name the HTLC script".to_string(),
            ));
        }
        if !lock.asset_id.starts_with("rgb:") {
            return Err(Error::Protocol(format!(
                "RGB asset id {} is not a contract id",
                lock.asset_id
            )));
        }
        if lock.amount == 0 {
            return Err(Error::Protocol("RGB lock amount is zero".to_string()));
        }
        if lock.htlc_sat < MIN_COLORED_OUTPUT_SAT {
            return Err(Error::Protocol(format!(
                "RGB HTLC carries {} sat, below the {MIN_COLORED_OUTPUT_SAT} sat minimum",
                lock.htlc_sat
            )));
        }
        if lock.min_confirmations == 0 {
            return Err(Error::Protocol(
                "RGB lock must require at least one confirmation".to_string(),
            ));
        }
        if lock.transport_endpoints.is_empty()
            || !lock
                .transport_endpoints
                .iter()
                .all(|e| e.starts_with("rpc://") || e.starts_with("rpcs://"))
        {
            return Err(Error::Protocol(
                "RGB transport endpoints must be rpc:// or rpcs:// proxies".to_string(),
            ));
        }
        if lock.claim_fee_rate == Some(0) {
            return Err(Error::Protocol("RGB claim fee rate is zero".to_string()));
        }
        let blinding = lock
            .blinding
            .parse::<u64>()
            .map_err(|_| Error::Protocol("RGB blinding is not a u64".to_string()))?;
        Ok(Self {
            cooperative_refund: lock.cooperative_refund.clone(),
            contract_id: lock.asset_id.clone(),
            amount: lock.amount,
            recipient_id: lock.recipient_id.clone(),
            recipient_network,
            blinding,
            htlc_sat: lock.htlc_sat,
            claim_fee_rate: lock.claim_fee_rate,
            script_pubkey,
            transport_endpoints: lock.transport_endpoints.clone(),
            min_confirmations: lock.min_confirmations,
        })
    }

    /// Bind this lock to the swap it came with: run after the Boltz checks
    /// (hashlock, timelock, address) passed on `swap_script`.
    pub(crate) fn validate_swap(
        &self,
        swap_script: &BtcSwapScript,
        chain: BitcoinChain,
        swap_address: &str,
        swap_amount: u64,
        expected_contract_id: &str,
    ) -> Result<(), Error> {
        if !same_contract_id(&self.contract_id, expected_contract_id) {
            return Err(Error::Protocol(format!(
                "RGB contract mismatch: the swap locks {}, expected {expected_contract_id}",
                self.contract_id
            )));
        }
        let address = Address::from_str(swap_address)?.assume_checked();
        if self.script_pubkey != address.script_pubkey() {
            return Err(Error::Protocol(
                "RGB scriptPubkey is not the swap address".to_string(),
            ));
        }
        if !self.recipient_network.is_compatible_with(chain) {
            return Err(Error::Protocol(format!(
                "RGB recipient id is on {}, not on {chain:?}",
                self.recipient_network.prefix()
            )));
        }
        if self.amount != swap_amount {
            return Err(Error::Protocol(format!(
                "RGB lock amount {} differs from the swap amount {swap_amount}",
                self.amount
            )));
        }
        if swap_script.swap_type == SwapType::ReverseSubmarine {
            let fee_rate = self.claim_fee_rate.ok_or_else(|| {
                Error::Protocol("RGB reverse swap without a claim fee rate".to_string())
            })?;
            let funded =
                self_funded_payout(SwapTxKind::Claim, swap_script, self.htlc_sat, fee_rate)?;
            if funded < MIN_COLORED_OUTPUT_SAT {
                return Err(Error::Protocol(format!(
                    "RGB HTLC of {} sat cannot fund the claim at {fee_rate} sat/vB",
                    self.htlc_sat
                )));
            }
        }
        Ok(())
    }
}

fn compact_size_len(n: u64) -> u64 {
    match n {
        0..=0xfc => 1,
        0xfd..=0xffff => 3,
        0x10000..=0xffff_ffff => 5,
        _ => 9,
    }
}

/// Weight of a claim-leaf witness `[sig, preimage, script, control block]`.
pub fn claim_leaf_witness_weight(script_len: usize) -> u64 {
    let len = script_len as u64;
    1 + (1 + 64) + (1 + 32) + (compact_size_len(len) + len) + (1 + CONTROL_BLOCK_BYTES)
}

/// Weight of a refund-leaf witness `[sig, script, control block]`.
pub fn refund_leaf_witness_weight(script_len: usize) -> u64 {
    let len = script_len as u64;
    1 + (1 + 64) + (compact_size_len(len) + len) + (1 + CONTROL_BLOCK_BYTES)
}

fn leaf_witness_weight(kind: SwapTxKind, swap_script: &BtcSwapScript) -> u64 {
    match kind {
        SwapTxKind::Claim => claim_leaf_witness_weight(swap_script.claim_script().len()),
        SwapTxKind::Refund => refund_leaf_witness_weight(swap_script.refund_script().len()),
    }
}

/// The unsigned spend: the HTLC input, the empty `OP_RETURN` the wallet writes
/// the commitment into, and the colored payout.
fn spend_skeleton(
    kind: SwapTxKind,
    swap_script: &BtcSwapScript,
    outpoint: OutPoint,
    payout_script: ScriptBuf,
    payout_value: u64,
) -> Transaction {
    Transaction {
        version: Version::TWO,
        lock_time: match kind {
            SwapTxKind::Claim => bitcoin::absolute::LockTime::ZERO,
            SwapTxKind::Refund => swap_script.locktime,
        },
        input: vec![TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: RGB_SPEND_SEQUENCE,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: Amount::ZERO,
                script_pubkey: ScriptBuf::new_op_return([]),
            },
            TxOut {
                value: Amount::from_sat(payout_value),
                script_pubkey: payout_script,
            },
        ],
    }
}

/// The fee of a spend whose only input is the HTLC, at `fee_rate` sat/vB.
///
/// Priced as the maker prices the taker claim it sizes reverse locks for:
/// the skeleton, the segwit marker and flag, the leaf witness and the
/// commitment's 32 bytes.
fn self_funded_fee(tx: &Transaction, witness_weight: u64, fee_rate: u64) -> Result<u64, Error> {
    if fee_rate == 0 {
        return Err(Error::Protocol(
            "RGB spend fee rate must be positive".to_string(),
        ));
    }
    let weight = tx.weight()
        + Weight::from_wu(2 + witness_weight)
        + Weight::from_non_witness_data_size(COMMITMENT_BYTES as u64);
    weight
        .to_vbytes_ceil()
        .checked_mul(fee_rate)
        .ok_or_else(|| Error::Protocol("RGB spend fee overflows".to_string()))
}

/// What a self-funded spend of `htlc_sat` leaves for the colored payout at
/// `fee_rate`, priced against a P2TR payout. Zero when the fee eats it all.
fn self_funded_payout(
    kind: SwapTxKind,
    swap_script: &BtcSwapScript,
    htlc_sat: u64,
    fee_rate: u64,
) -> Result<u64, Error> {
    let placeholder = ScriptBuf::new_p2tr_tweaked(TweakedPublicKey::dangerous_assume_tweaked(
        XOnlyPublicKey::from_slice(&[1; 32]).expect("valid x-only key"),
    ));
    let tx = spend_skeleton(kind.clone(), swap_script, OutPoint::null(), placeholder, 0);
    let fee = self_funded_fee(&tx, leaf_witness_weight(kind, swap_script), fee_rate)?;
    Ok(htlc_sat.saturating_sub(fee))
}

/// How an RGB spend pays its miner fee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbSpendFunding {
    /// Out of the HTLC's own sats: `[HTLC] → [OP_RETURN, payout]`, with the
    /// payout carrying `htlcSat − fee`. This is the shape a reverse lock's
    /// `htlcSat` is sized for at `claimFeeRate`.
    HtlcValue { fee_rate_sat_vb: u64 },
    /// From inputs the caller's wallet adds: `[HTLC, wallet inputs…] →
    /// [OP_RETURN, payout, wallet change…]`. The payout keeps the whole HTLC
    /// value. Call [`PreparedRgbSpend::fund`] before the wallet colors it.
    CallerInputs,
}

/// The spend handed to the caller's RGB wallet for funding and coloring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbPsbtTemplate {
    /// Base64 PSBT. The HTLC input has `witness_utxo` and no signature.
    pub psbt: String,
    /// The HTLC outpoint (`txid:vout`): the wallet's coloring input.
    pub swap_outpoint: String,
    pub swap_input_index: u32,
    /// The empty `OP_RETURN` the wallet writes the commitment into.
    pub commitment_output_index: u32,
    /// The colored payout; the transition must assign `amount` to it alone.
    pub payment_output_index: u32,
    pub asset_id: String,
    pub amount: u64,
    pub payment_value: u64,
    /// The highest fee the SDK will sign, in sats.
    pub max_fee: u64,
    /// The wallet adds fee inputs and change, after the payout, before it
    /// colors; then hands the funded PSBT to [`PreparedRgbSpend::fund`].
    pub requires_funding: bool,
}

/// One assignment rgb-lib's `psbt_op_prepare_with_expiry` reports for the
/// colored spend (`PsbtOpPrepareResult::allocations`), with a fungible
/// assignment's amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbAllocation {
    pub asset_id: String,
    pub vout: Option<u32>,
    pub amount: u64,
}

/// What the wallet's coloring returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColoredRgbPsbt {
    /// Base64 PSBT after `psbt_op_prepare_with_expiry`.
    pub psbt: String,
    pub allocations: Vec<RgbAllocation>,
}

/// A signed RGB spend.
#[derive(Debug, Clone)]
pub struct FinalizedRgbSpend {
    /// The colored PSBT with the HTLC input's `final_script_witness` set.
    /// The wallet signs and finalizes its own inputs on it, and keeps the
    /// HTLC witness exactly as it is.
    pub psbt: Psbt,
    pub swap_input_index: u32,
    /// The broadcastable transaction, when every input is already final:
    /// always for [`RgbSpendFunding::HtlcValue`].
    pub transaction: Option<Transaction>,
}

/// An immutable RGB HTLC spend: the template, the funding it accepted, and
/// the checks the wallet's coloring must pass before the SDK signs.
///
/// Mirrors the maker's own colored spends: coloring may only write the
/// commitment into output 0, and the transition must assign exactly the
/// locked amount to output 1.
#[derive(Debug, Clone)]
pub struct PreparedRgbSpend {
    cooperative: bool,
    kind: SwapTxKind,
    swap_script: BtcSwapScript,
    context: RgbHtlcContext,
    funding_outpoint: OutPoint,
    funding_utxo: TxOut,
    funding: RgbSpendFunding,
    max_fee: u64,
    template: Psbt,
    /// The funded, uncolored PSBT, once [`Self::fund`] accepted it.
    funded: Option<Psbt>,
}

pub const RGB_COOPERATIVE_REFUND_PROTOCOL: &str = "rgb-coop-refund-v1";

/// Versioned RGB request for the existing submarine refund endpoint.
/// The PSBT contains the committed RGB transition/proof; history is held by the maker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RgbCooperativeRefundRequest {
    pub protocol: String,
    pub psbt: String,
    pub index: u32,
    pub pub_nonce: String,
    pub session_id: String,
}

impl RgbCooperativeRefundRequest {
    pub fn request_hash(&self, swap_id: &str) -> String {
        let mut engine = sha256::Hash::engine();
        engine.input(b"kaleidoswap/rgb-coop-refund/v1\0");
        for value in [
            swap_id,
            &self.protocol,
            &self.session_id,
            &self.psbt,
            &self.pub_nonce,
        ] {
            engine.input(&(value.len() as u32).to_le_bytes());
            engine.input(value.as_bytes());
        }
        engine.input(&self.index.to_le_bytes());
        sha256::Hash::from_engine(engine).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbCooperativeRefundResponse {
    pub session_id: String,
    pub request_hash: String,
    pub pub_nonce: String,
    pub partial_signature: String,
}

/// A non-cloneable, non-serializable secret nonce. Completing consumes the session.
pub struct RgbCooperativeRefund {
    psbt: Psbt,
    request: RgbCooperativeRefundRequest,
    request_hash: String,
    index: usize,
    message: [u8; 32],
    key_agg: musig::KeyAggCache,
    secret_nonce: musig::SecretNonce,
    public_nonce: musig::PublicNonce,
    swap_script: BtcSwapScript,
}

impl std::fmt::Debug for RgbCooperativeRefund {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgbCooperativeRefund")
            .field("session_id", &self.request.session_id)
            .finish_non_exhaustive()
    }
}

impl RgbCooperativeRefund {
    pub fn request(&self) -> RgbCooperativeRefundRequest {
        self.request.clone()
    }

    pub fn complete(
        self,
        response: RgbCooperativeRefundResponse,
        keys: &Keypair,
    ) -> Result<FinalizedRgbSpend, Error> {
        if response.session_id != self.request.session_id
            || response.request_hash != self.request_hash
        {
            return Err(Error::Protocol(
                "RGB cooperative reply belongs to another signing context".into(),
            ));
        }
        if keys.public_key() != self.swap_script.sender_pubkey.inner {
            return Err(Error::Protocol(
                "RGB cooperative refund requires the taker's refund key".into(),
            ));
        }
        let nonce = musig::PublicNonce::from_str(&response.pub_nonce)?;
        let partial = musig::PartialSignature::from_str(&response.partial_signature)?;
        let aggregate_nonce = musig::AggregatedNonce::new(&[&nonce, &self.public_nonce]);
        let session = musig::Session::new(&self.key_agg, aggregate_nonce, &self.message);
        if !session.partial_verify(
            &self.key_agg,
            &partial,
            &nonce,
            convert_public_key(self.swap_script.receiver_pubkey.inner),
        ) {
            return Err(Error::Protocol(
                "Invalid maker partial signature for RGB refund".into(),
            ));
        }
        let own = session.partial_sign(self.secret_nonce, &convert_keypair(keys), &self.key_agg);
        let signature =
            convert_schnorr_signature(session.partial_sig_agg(&[&partial, &own]).assume_valid());
        Secp256k1::new().verify_schnorr(
            &signature,
            &Message::from_digest(self.message),
            &self
                .swap_script
                .taproot_spendinfo()?
                .output_key()
                .to_x_only_public_key(),
        )?;
        let mut psbt = self.psbt;
        psbt.inputs[self.index].final_script_witness =
            Some(Witness::from_slice(&[signature.as_ref()]));
        let transaction = psbt
            .inputs
            .iter()
            .all(|input| input.final_script_witness.is_some())
            .then(|| {
                let mut tx = psbt.unsigned_tx.clone();
                for (txin, input) in tx.input.iter_mut().zip(&psbt.inputs) {
                    txin.witness = input.final_script_witness.clone().unwrap_or_default();
                }
                tx
            });
        Ok(FinalizedRgbSpend {
            psbt,
            swap_input_index: self.index as u32,
            transaction,
        })
    }
}

fn psbt_from_base64(psbt: &str) -> Result<Psbt, Error> {
    Psbt::from_str(psbt).map_err(|e| Error::Protocol(format!("Invalid PSBT: {e}")))
}

impl PreparedRgbSpend {
    pub fn new(
        kind: SwapTxKind,
        swap_script: BtcSwapScript,
        output_address: &str,
        network: BitcoinChain,
        (funding_outpoint, funding_utxo): (OutPoint, TxOut),
        funding: RgbSpendFunding,
        max_fee: u64,
    ) -> Result<Self, Error> {
        let context = swap_script.rgb.clone().ok_or_else(|| {
            Error::Protocol("RGB spend requested for a swap without an RGB lock".to_string())
        })?;
        match (&kind, swap_script.swap_type) {
            (SwapTxKind::Claim, SwapType::ReverseSubmarine)
            | (SwapTxKind::Refund, SwapType::Submarine) => {}
            (kind, swap_type) => {
                return Err(Error::Protocol(format!(
                    "RGB {kind:?} is not the taker's spend of a {swap_type:?} swap"
                )))
            }
        }
        if funding_utxo.script_pubkey != context.script_pubkey {
            return Err(Error::Protocol(
                "RGB spend input is not the swap HTLC".to_string(),
            ));
        }
        let htlc_value = funding_utxo.value.to_sat();
        if kind == SwapTxKind::Claim && htlc_value != context.htlc_sat {
            return Err(Error::Protocol(format!(
                "RGB HTLC carries {htlc_value} sat, the swap locked {}",
                context.htlc_sat
            )));
        }
        let payout_script = Address::from_str(output_address)?
            .require_network(network.into())
            .map_err(|_| {
                Error::Address(format!("RGB payout address is not valid for {network:?}"))
            })?
            .script_pubkey();

        let mut tx = spend_skeleton(
            kind.clone(),
            &swap_script,
            funding_outpoint,
            payout_script,
            htlc_value,
        );
        if let RgbSpendFunding::HtlcValue { fee_rate_sat_vb } = funding {
            let fee = self_funded_fee(
                &tx,
                leaf_witness_weight(kind.clone(), &swap_script),
                fee_rate_sat_vb,
            )?;
            if fee > max_fee {
                return Err(Error::Protocol(format!(
                    "RGB spend fee {fee} sat exceeds the {max_fee} sat cap"
                )));
            }
            match htlc_value.checked_sub(fee) {
                Some(payout) if payout >= MIN_COLORED_OUTPUT_SAT => {
                    tx.output[RGB_PAYMENT_OUTPUT_INDEX as usize].value = Amount::from_sat(payout)
                }
                _ => return Err(Error::RgbFeeInputRequired),
            }
        } else if htlc_value < MIN_COLORED_OUTPUT_SAT {
            return Err(Error::Protocol(format!(
                "RGB HTLC carries {htlc_value} sat, below the {MIN_COLORED_OUTPUT_SAT} sat \
                 colored-output minimum"
            )));
        }

        let mut template = Psbt::from_unsigned_tx(tx)
            .map_err(|e| Error::Protocol(format!("RGB spend PSBT: {e}")))?;
        template.inputs[0].witness_utxo = Some(funding_utxo.clone());

        Ok(Self {
            cooperative: false,
            kind,
            swap_script,
            context,
            funding_outpoint,
            funding_utxo,
            funding,
            max_fee,
            template,
            funded: None,
        })
    }

    /// Prepare an early submarine refund. Funding/coloring precede MuSig2 signing.
    pub fn new_cooperative_refund(
        swap_script: BtcSwapScript,
        output_address: &str,
        network: BitcoinChain,
        funding_utxo: (OutPoint, TxOut),
        funding: RgbSpendFunding,
        max_fee: u64,
    ) -> Result<Self, Error> {
        let mut prepared = Self::new(
            SwapTxKind::Refund,
            swap_script,
            output_address,
            network,
            funding_utxo,
            RgbSpendFunding::CallerInputs,
            max_fee,
        )?;
        if prepared.context.cooperative_refund.as_deref() != Some(RGB_COOPERATIVE_REFUND_PROTOCOL) {
            return Err(Error::Protocol(
                "Maker does not advertise RGB cooperative refunds".into(),
            ));
        }
        prepared.cooperative = true;
        prepared.funding = funding;
        prepared.template.unsigned_tx.lock_time = bitcoin::absolute::LockTime::ZERO;
        if let RgbSpendFunding::HtlcValue { fee_rate_sat_vb } = funding {
            let fee = self_funded_fee(&prepared.template.unsigned_tx, 66, fee_rate_sat_vb)?;
            if fee > max_fee {
                return Err(Error::Protocol(
                    "RGB cooperative refund exceeds the fee cap".into(),
                ));
            }
            let payout = prepared
                .funding_utxo
                .value
                .to_sat()
                .checked_sub(fee)
                .filter(|value| *value >= MIN_COLORED_OUTPUT_SAT)
                .ok_or(Error::RgbFeeInputRequired)?;
            prepared.template.unsigned_tx.output[RGB_PAYMENT_OUTPUT_INDEX as usize].value =
                Amount::from_sat(payout);
        }
        Ok(prepared)
    }

    /// Begin a one-use MuSig2 session after the local wallet has colored the frozen PSBT.
    /// Allocation metadata must come from the trusted wallet; the maker validates the proof.
    pub fn begin_cooperative_refund(
        &self,
        colored: ColoredRgbPsbt,
        keys: &Keypair,
        swap_id: &str,
    ) -> Result<RgbCooperativeRefund, Error> {
        if !self.cooperative || self.kind != SwapTxKind::Refund {
            return Err(Error::Protocol(
                "Prepare an RGB cooperative refund before signing it".into(),
            ));
        }
        if keys.public_key() != self.swap_script.sender_pubkey.inner {
            return Err(Error::Protocol(
                "RGB cooperative refund requires the taker's refund key".into(),
            ));
        }
        if swap_id.is_empty() || swap_id.len() > 128 {
            return Err(Error::Protocol("Invalid RGB cooperative swap id".into()));
        }
        let (psbt, index, prevouts) = self.validate_colored(colored)?;
        let message = SighashCache::new(&psbt.unsigned_tx)
            .taproot_key_spend_signature_hash(
                index,
                &Prevouts::All(&prevouts),
                TapSighashType::Default,
            )?
            .to_byte_array();
        let mut key_agg = self.swap_script.musig_keyagg_cache();
        let tweak = Scalar::from_be_bytes(
            self.swap_script
                .taproot_spendinfo()?
                .tap_tweak()
                .to_byte_array(),
        )?;
        key_agg.pubkey_xonly_tweak_add(&tweak)?;
        let (secret_nonce, public_nonce) = key_agg.nonce_gen(
            musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b()),
            convert_public_key(keys.public_key()),
            &message,
            Some(rng_32b()),
        );
        let request = RgbCooperativeRefundRequest {
            protocol: RGB_COOPERATIVE_REFUND_PROTOCOL.to_owned(),
            psbt: psbt.to_string(),
            index: index as u32,
            pub_nonce: public_nonce.to_string(),
            session_id: bitcoin::hex::DisplayHex::to_lower_hex_string(&rng_32b()),
        };
        let request_hash = request.request_hash(swap_id);
        Ok(RgbCooperativeRefund {
            psbt,
            request,
            request_hash,
            index,
            message,
            key_agg,
            secret_nonce,
            public_nonce,
            swap_script: self.swap_script.clone(),
        })
    }

    pub fn template(&self) -> RgbPsbtTemplate {
        let psbt = self.funded.as_ref().unwrap_or(&self.template);
        let tx = &psbt.unsigned_tx;
        RgbPsbtTemplate {
            psbt: psbt.to_string(),
            swap_outpoint: self.funding_outpoint.to_string(),
            swap_input_index: self
                .htlc_input_index(tx)
                .expect("validated RGB PSBT retains its HTLC input")
                as u32,
            commitment_output_index: RGB_COMMITMENT_OUTPUT_INDEX,
            payment_output_index: RGB_PAYMENT_OUTPUT_INDEX,
            asset_id: self.context.contract_id.clone(),
            amount: self.context.amount,
            payment_value: tx.output[RGB_PAYMENT_OUTPUT_INDEX as usize].value.to_sat(),
            max_fee: self.max_fee,
            requires_funding: self.funding == RgbSpendFunding::CallerInputs
                && self.funded.is_none(),
        }
    }

    /// Accept the wallet's funding of a [`RgbSpendFunding::CallerInputs`]
    /// spend, before it is colored, and freeze it.
    ///
    /// The wallet may add inputs, and outputs after the payout. It must not
    /// touch the HTLC input, the `OP_RETURN` or the payout. Every input needs
    /// `witness_utxo`, because the HTLC signature commits to all of them.
    pub fn fund(&self, funded_psbt: &str) -> Result<Self, Error> {
        if self.funding != RgbSpendFunding::CallerInputs {
            return Err(Error::Protocol(
                "This RGB spend pays its fee from the HTLC and takes no funding".to_string(),
            ));
        }
        if self.funded.is_some() {
            return Err(Error::Protocol(
                "This RGB spend is already funded".to_string(),
            ));
        }
        let funded = psbt_from_base64(funded_psbt)?;
        let tx = &funded.unsigned_tx;
        let template = &self.template.unsigned_tx;
        if tx.input.len() > MAX_RGB_PSBT_INPUTS || tx.output.len() > MAX_RGB_PSBT_OUTPUTS {
            return Err(Error::Protocol("Funded RGB PSBT is too large".to_string()));
        }
        if tx.version != template.version || tx.lock_time != template.lock_time {
            return Err(Error::Protocol(
                "Funding changed the RGB spend's version or locktime".to_string(),
            ));
        }
        if tx.output.len() < 2 || tx.output[..2] != template.output[..] {
            return Err(Error::Protocol(
                "Funding changed the RGB commitment or payout output".to_string(),
            ));
        }
        if tx.output[2..]
            .iter()
            .any(|o| o.script_pubkey.is_op_return())
        {
            return Err(Error::Protocol(
                "Funding added a second OP_RETURN to the RGB spend".to_string(),
            ));
        }
        let htlc_index = self.htlc_input_index(tx)?;
        if tx.input[htlc_index] != template.input[0]
            || funded.inputs[htlc_index].witness_utxo.as_ref() != Some(&self.funding_utxo)
        {
            return Err(Error::Protocol(
                "Funding changed the RGB spend's HTLC input".to_string(),
            ));
        }
        if htlc_input_is_signed(&funded, htlc_index) {
            return Err(Error::Protocol(
                "The HTLC input is already signed".to_string(),
            ));
        }
        let prevouts = input_prevouts(&funded)?;
        let fee = spend_fee(tx, &prevouts)?;
        if fee == 0 || fee > self.max_fee {
            return Err(Error::Protocol(format!(
                "Funded RGB spend pays {fee} sat in fees, outside (0, {}]",
                self.max_fee
            )));
        }
        let mut prepared = self.clone();
        prepared.funded = Some(funded);
        Ok(prepared)
    }

    /// Sign the claim leaf of a colored reverse-swap claim.
    pub fn finalize_claim(
        &self,
        colored: ColoredRgbPsbt,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<FinalizedRgbSpend, Error> {
        if self.kind != SwapTxKind::Claim {
            return Err(Error::Protocol("This RGB spend is a refund".to_string()));
        }
        self.finalize(colored, keys, Some(preimage))
    }

    /// Sign the refund leaf of a colored submarine-swap refund. Broadcast it
    /// only after the swap's timeout block.
    pub fn finalize_refund(
        &self,
        colored: ColoredRgbPsbt,
        keys: &Keypair,
    ) -> Result<FinalizedRgbSpend, Error> {
        if self.cooperative {
            return Err(Error::Protocol(
                "Use the cooperative session to finalize this RGB refund".into(),
            ));
        }
        if self.kind != SwapTxKind::Refund {
            return Err(Error::Protocol("This RGB spend is a claim".to_string()));
        }
        self.finalize(colored, keys, None)
    }

    fn finalize(
        &self,
        colored: ColoredRgbPsbt,
        keys: &Keypair,
        preimage: Option<&Preimage>,
    ) -> Result<FinalizedRgbSpend, Error> {
        let (mut psbt, htlc_index, prevouts) = self.validate_colored(colored)?;
        let witness = self.swap_script.rgb_leaf_witness(
            self.kind.clone(),
            &psbt.unsigned_tx,
            htlc_index,
            &prevouts,
            keys,
            preimage,
        )?;
        psbt.inputs[htlc_index].witness_utxo = Some(self.funding_utxo.clone());
        psbt.inputs[htlc_index].final_script_witness = Some(witness);

        let transaction = psbt
            .inputs
            .iter()
            .all(|input| input.final_script_witness.is_some() || input.final_script_sig.is_some())
            .then(|| {
                let mut tx = psbt.unsigned_tx.clone();
                for (txin, input) in tx.input.iter_mut().zip(&psbt.inputs) {
                    txin.witness = input.final_script_witness.clone().unwrap_or_default();
                    txin.script_sig = input.final_script_sig.clone().unwrap_or_default();
                }
                tx
            });

        Ok(FinalizedRgbSpend {
            psbt,
            swap_input_index: htlc_index as u32,
            transaction,
        })
    }

    fn validate_colored(
        &self,
        colored: ColoredRgbPsbt,
    ) -> Result<(Psbt, usize, Vec<TxOut>), Error> {
        let frozen = match (&self.funding, &self.funded) {
            (RgbSpendFunding::HtlcValue { .. }, _) => &self.template,
            (RgbSpendFunding::CallerInputs, Some(funded)) => funded,
            (RgbSpendFunding::CallerInputs, None) => {
                return Err(Error::Protocol(
                    "Fund this RGB spend before coloring and signing it".to_string(),
                ))
            }
        };
        let psbt = psbt_from_base64(&colored.psbt)?;
        ensure_only_commitment_added(&frozen.unsigned_tx, &psbt.unsigned_tx)?;

        let expected = RgbAllocation {
            asset_id: self.context.contract_id.clone(),
            vout: Some(RGB_PAYMENT_OUTPUT_INDEX),
            amount: self.context.amount,
        };
        match colored.allocations.as_slice() {
            [allocation]
                if same_contract_id(&allocation.asset_id, &expected.asset_id)
                    && allocation.vout == expected.vout
                    && allocation.amount == expected.amount => {}
            other => {
                return Err(Error::Protocol(format!(
                    "The RGB transition assigns {other:?}, not {} of {} to output {}",
                    expected.amount, expected.asset_id, RGB_PAYMENT_OUTPUT_INDEX
                )))
            }
        }

        let htlc_index = self.htlc_input_index(&psbt.unsigned_tx)?;
        if htlc_input_is_signed(&psbt, htlc_index) {
            return Err(Error::Protocol(
                "The HTLC input is already signed".to_string(),
            ));
        }
        // The sighash commits to every prevout. Take them from the frozen,
        // validated PSBT, not from what came back with the coloring.
        let prevouts = input_prevouts(frozen)?;
        if input_prevouts(&psbt)? != prevouts {
            return Err(Error::Protocol(
                "RGB coloring changed previous-output metadata".into(),
            ));
        }
        Ok((psbt, htlc_index, prevouts))
    }

    fn htlc_input_index(&self, tx: &Transaction) -> Result<usize, Error> {
        let mut seen = HashSet::new();
        if !tx.input.iter().all(|i| seen.insert(i.previous_output)) {
            return Err(Error::Protocol(
                "RGB spend has a duplicate input".to_string(),
            ));
        }
        tx.input
            .iter()
            .position(|i| i.previous_output == self.funding_outpoint)
            .ok_or_else(|| Error::Protocol("RGB spend does not spend the HTLC".to_string()))
    }
}

fn htlc_input_is_signed(psbt: &Psbt, index: usize) -> bool {
    let input = &psbt.inputs[index];
    input.final_script_witness.is_some()
        || input.final_script_sig.is_some()
        || !psbt.unsigned_tx.input[index].witness.is_empty()
}

/// Every input's previous output, from `witness_utxo`, which must agree with
/// any `non_witness_utxo` the wallet also supplied.
fn input_prevouts(psbt: &Psbt) -> Result<Vec<TxOut>, Error> {
    psbt.unsigned_tx
        .input
        .iter()
        .zip(&psbt.inputs)
        .map(|(txin, input)| {
            let prevout = input.witness_utxo.clone().ok_or_else(|| {
                Error::Protocol(format!(
                    "RGB spend input {} has no witness_utxo",
                    txin.previous_output
                ))
            })?;
            if !(prevout.script_pubkey.is_p2tr() || prevout.script_pubkey.is_p2wpkh())
                || !txin.script_sig.is_empty()
                || input
                    .final_script_sig
                    .as_ref()
                    .is_some_and(|script| !script.is_empty())
            {
                return Err(Error::Protocol(
                    "RGB fee inputs must use native P2WPKH or P2TR without scriptSig".into(),
                ));
            }
            if let Some(prev_tx) = &input.non_witness_utxo {
                let matches = prev_tx.compute_txid() == txin.previous_output.txid
                    && prev_tx.output.get(txin.previous_output.vout as usize) == Some(&prevout);
                if !matches {
                    return Err(Error::Protocol(format!(
                        "RGB spend input {} has inconsistent previous outputs",
                        txin.previous_output
                    )));
                }
            }
            Ok(prevout)
        })
        .collect()
}

fn spend_fee(tx: &Transaction, prevouts: &[TxOut]) -> Result<u64, Error> {
    fn total<'a>(outputs: impl IntoIterator<Item = &'a TxOut>) -> Option<Amount> {
        outputs
            .into_iter()
            .try_fold(Amount::ZERO, |acc, o| acc.checked_add(o.value))
    }
    let inputs = total(prevouts);
    let outputs = total(&tx.output);
    match (inputs, outputs) {
        (Some(inputs), Some(outputs)) if inputs >= outputs => Ok((inputs - outputs).to_sat()),
        _ => Err(Error::Protocol(
            "RGB spend outputs exceed its inputs".to_string(),
        )),
    }
}

/// The wallet's coloring may write the commitment into output 0, and change
/// nothing else.
fn ensure_only_commitment_added(frozen: &Transaction, colored: &Transaction) -> Result<(), Error> {
    let commitment = &colored.output.first().map(|o| o.script_pubkey.as_bytes());
    let committed = matches!(
        commitment,
        Some([0x6a, 0x20, rest @ ..]) if rest.len() == COMMITMENT_BYTES
    );
    let same_outputs = frozen.output.len() == colored.output.len()
        && frozen.output.first().map(|o| o.value) == colored.output.first().map(|o| o.value)
        && frozen.output.get(1..) == colored.output.get(1..);
    if colored.version != frozen.version
        || colored.lock_time != frozen.lock_time
        || colored.input != frozen.input
        || !same_outputs
    {
        return Err(Error::Protocol(
            "RGB coloring changed more than the commitment".to_string(),
        ));
    }
    if !committed {
        return Err(Error::Protocol(
            "RGB spend has no 32-byte commitment at output 0: it would burn the asset".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::sighash::{Prevouts, SighashCache};
    use bitcoin::taproot::{LeafVersion, Signature};
    use bitcoin::{PublicKey, TapLeafHash, TapSighashType, Txid};

    use crate::swaps::bitcoin::BtcSwapTx;

    /// Generated with rgb-invoicing 0.11.1-rc.11 (the encoder rgb-lib's
    /// `recipient_id_from_script_buf` uses): `XChainNet::with(net,
    /// Beneficiary::WitnessVout(Pay2Vout::new(AddressPayload::from_script(s)),
    /// None)).to_string()`.
    const RECIPIENT_VECTORS: &[(&str, &str)] = &[
        (
            "51200101010101010101010101010101010101010101010101010101010101010101",
            "wvout:BQEBAQEB-AQEBAQE-BAQEBAQ-EBAQEBA-QEBAQEB-AQEBAQE-Bv7a27w",
        ),
        (
            "512079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            "wvout:BXm_Zn75-3LusVaB-ilc6HCw-cCm~zbL-c4o2Vny-gVsW_Be-YRxgY5w",
        ),
        (
            "5120c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
            "wvout:BcYEf5RB-7X1tMEV-AbpXAfN-hcd45Lj-O88p6us-CblccJ7-lgMPDUw",
        ),
    ];

    const NETWORKS: &[RgbChainNet] = &[
        RgbChainNet::BitcoinMainnet,
        RgbChainNet::BitcoinTestnet3,
        RgbChainNet::BitcoinTestnet4,
        RgbChainNet::BitcoinSignet,
        RgbChainNet::BitcoinSignetCustom,
        RgbChainNet::BitcoinRegtest,
    ];

    #[test]
    fn recipient_ids_match_rgb_invoicing() {
        for (script_hex, body) in RECIPIENT_VECTORS {
            let script = ScriptBuf::from_hex(script_hex).unwrap();
            for net in NETWORKS {
                let expected = format!("{}:{body}", net.prefix());
                assert_eq!(rgb_recipient_id(&script, *net).unwrap(), expected);
                assert_eq!(
                    rgb_recipient_script(&expected).unwrap(),
                    (*net, script.clone())
                );
            }
        }
    }

    #[test]
    fn recipient_ids_refuse_anything_but_the_canonical_p2tr_form() {
        let (_, body) = RECIPIENT_VECTORS[1];
        let canonical = format!("bcrt:{body}");
        let refused = [
            // No dashes: the same bytes, a spelling rgb-lib never writes.
            canonical.replace('-', ""),
            format!("{canonical}+79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
            format!("lq:{body}"),
            format!("bcrt:{}", body.replace("wvout", "utxob")),
            // A flipped checksum character.
            canonical.replace("Y5w", "Y5x"),
            "bcrt".to_string(),
        ];
        for id in refused {
            assert!(rgb_recipient_script(&id).is_err(), "accepted {id}");
        }
        let p2wpkh = ScriptBuf::from_hex("00140101010101010101010101010101010101010101").unwrap();
        assert!(rgb_recipient_id(&p2wpkh, RgbChainNet::BitcoinRegtest).is_err());
    }

    #[test]
    fn recipient_networks_follow_the_swap_chain() {
        use RgbChainNet::*;
        assert!(BitcoinSignetCustom.is_compatible_with(BitcoinChain::BitcoinSignet));
        assert!(BitcoinTestnet4.is_compatible_with(BitcoinChain::BitcoinTestnet));
        assert!(!BitcoinMainnet.is_compatible_with(BitcoinChain::BitcoinRegtest));
        assert!(!BitcoinRegtest.is_compatible_with(BitcoinChain::Bitcoin));
    }

    struct Fixture {
        script: BtcSwapScript,
        taker: Keypair,
        preimage: Preimage,
        outpoint: OutPoint,
        utxo: TxOut,
    }

    const CONTRACT: &str = "rgb:WM~4iI3a-IIHtpsc-UoTNdky-~aYnGmB-~KF3h4r-BBd0zqQ";

    /// A P2TR payout on `network`, standing in for an rgb-lib `witness_receive`.
    fn dest_on(network: bitcoin::Network) -> String {
        let key = XOnlyPublicKey::from_slice(&[2; 32]).unwrap();
        Address::p2tr_tweaked(TweakedPublicKey::dangerous_assume_tweaked(key), network).to_string()
    }

    fn dest() -> String {
        dest_on(bitcoin::Network::Regtest)
    }

    fn fixture(swap_type: SwapType, htlc_sat: u64) -> Fixture {
        let secp = Secp256k1::new();
        let taker = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&[7; 32]).unwrap());
        let maker = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&[9; 32]).unwrap());
        let preimage = Preimage::from_vec([3; 32].to_vec()).unwrap();
        let (receiver, sender) = match swap_type {
            SwapType::ReverseSubmarine => (taker, maker),
            _ => (maker, taker),
        };
        let mut script = BtcSwapScript {
            swap_type,
            side: None,
            funding_addrs: None,
            hashlock: preimage.hash160,
            receiver_pubkey: PublicKey::new(receiver.public_key()),
            locktime: bitcoin::absolute::LockTime::from_height(500).unwrap(),
            sender_pubkey: PublicKey::new(sender.public_key()),
            expected_amount: htlc_sat,
            rgb: None,
        };
        let script_pubkey = script
            .to_address(BitcoinChain::BitcoinRegtest)
            .unwrap()
            .script_pubkey();
        script.rgb = Some(RgbHtlcContext {
            cooperative_refund: None,
            contract_id: CONTRACT.to_string(),
            amount: 1_005,
            recipient_id: rgb_recipient_id(&script_pubkey, RgbChainNet::BitcoinRegtest).unwrap(),
            recipient_network: RgbChainNet::BitcoinRegtest,
            blinding: 42,
            htlc_sat,
            claim_fee_rate: Some(5),
            script_pubkey: script_pubkey.clone(),
            transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".to_string()],
            min_confirmations: 1,
        });
        Fixture {
            script,
            taker,
            preimage,
            outpoint: OutPoint::new(Txid::from_byte_array([5; 32]), 1),
            utxo: TxOut {
                value: Amount::from_sat(htlc_sat),
                script_pubkey,
            },
        }
    }

    fn prepare(f: &Fixture, kind: SwapTxKind, funding: RgbSpendFunding) -> PreparedRgbSpend {
        PreparedRgbSpend::new(
            kind,
            f.script.clone(),
            &dest(),
            BitcoinChain::BitcoinRegtest,
            (f.outpoint, f.utxo.clone()),
            funding,
            10_000,
        )
        .unwrap()
    }

    /// What `psbt_op_prepare_with_expiry` does to the transaction.
    fn color(psbt: &str) -> String {
        let mut psbt = Psbt::from_str(psbt).unwrap();
        psbt.unsigned_tx.output[0].script_pubkey = ScriptBuf::new_op_return([0xab; 32]);
        psbt.to_string()
    }

    fn allocations() -> Vec<RgbAllocation> {
        vec![RgbAllocation {
            asset_id: CONTRACT.to_string(),
            vout: Some(1),
            amount: 1_005,
        }]
    }

    fn colored(psbt: &str) -> ColoredRgbPsbt {
        ColoredRgbPsbt {
            psbt: color(psbt),
            allocations: allocations(),
        }
    }

    /// Check the leaf signature in `witness` the way consensus would.
    fn assert_valid_leaf_spend(tx: &Transaction, index: usize, prevouts: &[TxOut], key: &Keypair) {
        let witness = &tx.input[index].witness;
        let leaf = ScriptBuf::from_bytes(witness.nth(witness.len() - 2).unwrap().to_vec());
        let sighash = SighashCache::new(tx)
            .taproot_script_spend_signature_hash(
                index,
                &Prevouts::All(prevouts),
                TapLeafHash::from_script(&leaf, LeafVersion::TapScript),
                TapSighashType::Default,
            )
            .unwrap();
        let sig = Signature::from_slice(witness.nth(0).unwrap()).unwrap();
        let msg = bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array());
        Secp256k1::new()
            .verify_schnorr(&sig.signature, &msg, &key.x_only_public_key().0)
            .unwrap();
    }

    #[test]
    fn a_reverse_claim_leaf_prices_at_195_vbytes_like_the_maker() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        assert_eq!(f.script.claim_script().len(), 61);
        // 195 vB × 5 sat/vB + 546: the maker's `taker_claim_htlc_sat`.
        assert_eq!(
            self_funded_payout(SwapTxKind::Claim, &f.script, 1_521, 5).unwrap(),
            546
        );
        let prepared = prepare(
            &f,
            SwapTxKind::Claim,
            RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 5 },
        );
        assert_eq!(prepared.template().payment_value, 546);
        assert!(matches!(
            PreparedRgbSpend::new(
                SwapTxKind::Claim,
                f.script.clone(),
                &dest(),
                BitcoinChain::BitcoinRegtest,
                (f.outpoint, f.utxo.clone()),
                RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 6 },
                10_000,
            ),
            Err(Error::RgbFeeInputRequired)
        ));
    }

    #[test]
    fn a_self_funded_claim_signs_the_claim_leaf_over_the_colored_tx() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        let prepared = prepare(
            &f,
            SwapTxKind::Claim,
            RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 5 },
        );
        let template = prepared.template();
        assert_eq!(template.swap_outpoint, f.outpoint.to_string());
        assert!(!template.requires_funding);

        let spend = prepared
            .finalize_claim(colored(&template.psbt), &f.taker, &f.preimage)
            .unwrap();
        let tx = spend.transaction.expect("a single-input claim is final");
        assert_eq!(tx.lock_time, bitcoin::absolute::LockTime::ZERO);
        assert_eq!(tx.input[0].sequence, RGB_SPEND_SEQUENCE);
        assert_eq!(tx.input[0].witness.len(), 4);
        assert_eq!(tx.input[0].witness.nth(1).unwrap(), &[3; 32]);
        assert_valid_leaf_spend(&tx, 0, &[f.utxo.clone()], &f.taker);
        // The vsize the fee was priced at.
        assert_eq!(tx.vsize(), 195);
    }

    #[test]
    fn a_caller_funded_refund_signs_over_every_prevout() {
        let f = fixture(SwapType::Submarine, 1_000);
        let prepared = prepare(&f, SwapTxKind::Refund, RgbSpendFunding::CallerInputs);
        let template = prepared.template();
        assert!(template.requires_funding);
        assert_eq!(template.payment_value, 1_000);

        // The wallet puts its fee input first and its change last.
        let wallet_prevout = TxOut {
            value: Amount::from_sat(20_000),
            script_pubkey: Address::from_str(&dest())
                .unwrap()
                .assume_checked()
                .script_pubkey(),
        };
        let mut funded = Psbt::from_str(&template.psbt).unwrap();
        funded.unsigned_tx.input.insert(
            0,
            TxIn {
                previous_output: OutPoint::new(Txid::from_byte_array([8; 32]), 0),
                ..Default::default()
            },
        );
        funded.inputs.insert(
            0,
            bitcoin::psbt::Input {
                witness_utxo: Some(wallet_prevout.clone()),
                ..Default::default()
            },
        );
        funded.unsigned_tx.output.push(TxOut {
            value: Amount::from_sat(19_000),
            script_pubkey: wallet_prevout.script_pubkey.clone(),
        });
        funded.outputs.push(Default::default());

        assert!(prepared
            .finalize_refund(colored(&funded.to_string()), &f.taker)
            .is_err());
        let funded_spend = prepared.fund(&funded.to_string()).unwrap();
        let frozen = funded_spend.template();
        assert_eq!(frozen.psbt, funded.to_string());
        assert_eq!(frozen.swap_input_index, 1);
        assert!(!frozen.requires_funding);
        assert!(
            prepared.template().requires_funding,
            "fund leaves the original immutable"
        );
        let spend = funded_spend
            .finalize_refund(colored(&funded.to_string()), &f.taker)
            .unwrap();
        assert_eq!(spend.swap_input_index, 1);
        assert!(spend.transaction.is_none(), "the wallet input is unsigned");

        let mut tx = spend.psbt.unsigned_tx.clone();
        tx.input[1].witness = spend.psbt.inputs[1].final_script_witness.clone().unwrap();
        assert_eq!(tx.lock_time, f.script.locktime);
        assert_eq!(tx.input[1].witness.len(), 3);
        assert_valid_leaf_spend(&tx, 1, &[wallet_prevout, f.utxo.clone()], &f.taker);
    }

    #[test]
    fn funding_may_not_touch_the_htlc_the_commitment_or_the_payout() {
        let f = fixture(SwapType::Submarine, 1_000);
        let prepared = prepare(&f, SwapTxKind::Refund, RgbSpendFunding::CallerInputs);
        let template = Psbt::from_str(&prepared.template().psbt).unwrap();
        let wallet_input = |psbt: &mut Psbt, value: u64| {
            psbt.unsigned_tx.input.push(TxIn {
                previous_output: OutPoint::new(Txid::from_byte_array([8; 32]), 0),
                ..Default::default()
            });
            psbt.inputs.push(bitcoin::psbt::Input {
                witness_utxo: Some(TxOut {
                    value: Amount::from_sat(value),
                    script_pubkey: f.utxo.script_pubkey.clone(),
                }),
                ..Default::default()
            });
        };

        let mut ok = template.clone();
        wallet_input(&mut ok, 2_000);
        assert!(prepared.fund(&ok.to_string()).is_ok());

        let mut tampered: Vec<(&str, Psbt)> = Vec::new();
        let mut payout = ok.clone();
        payout.unsigned_tx.output[1].value = Amount::from_sat(900);
        tampered.push(("payout skimmed", payout));
        let mut fee = template.clone();
        wallet_input(&mut fee, 20_000);
        tampered.push(("fee above the cap", fee));
        tampered.push(("no fee", template.clone()));
        let mut second_op_return = ok.clone();
        second_op_return.unsigned_tx.output.push(TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::new_op_return([]),
        });
        second_op_return.outputs.push(Default::default());
        tampered.push(("second OP_RETURN", second_op_return));
        let mut sequence = ok.clone();
        sequence.unsigned_tx.input[0].sequence = Sequence::MAX;
        tampered.push(("HTLC sequence", sequence));
        let mut prevout = ok.clone();
        prevout.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(100_000);
        tampered.push(("HTLC prevout", prevout));
        let mut no_prevout = ok.clone();
        no_prevout.inputs[1].witness_utxo = None;
        tampered.push(("wallet input without witness_utxo", no_prevout));
        let mut duplicate = ok.clone();
        wallet_input(&mut duplicate, 1);
        tampered.push(("duplicate input", duplicate));
        let mut locktime = ok.clone();
        locktime.unsigned_tx.lock_time = bitcoin::absolute::LockTime::ZERO;
        tampered.push(("locktime", locktime));

        for (what, psbt) in tampered {
            assert!(prepared.fund(&psbt.to_string()).is_err(), "accepted {what}");
        }
    }

    #[test]
    fn finalize_refuses_coloring_beyond_the_commitment_and_wrong_allocations() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        let prepared = prepare(
            &f,
            SwapTxKind::Claim,
            RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 5 },
        );
        let template = prepared.template().psbt;
        let claim = |colored: ColoredRgbPsbt| {
            prepared
                .finalize_claim(colored, &f.taker, &f.preimage)
                .is_err()
        };

        // Uncolored: broadcasting this would burn the asset.
        assert!(claim(ColoredRgbPsbt {
            psbt: template.clone(),
            allocations: allocations(),
        }));
        let mut short = Psbt::from_str(&template).unwrap();
        short.unsigned_tx.output[0].script_pubkey = ScriptBuf::new_op_return([0xab; 31]);
        assert!(claim(ColoredRgbPsbt {
            psbt: short.to_string(),
            allocations: allocations(),
        }));
        let mut moved = Psbt::from_str(&color(&template)).unwrap();
        moved.unsigned_tx.output[1].value = Amount::from_sat(500);
        assert!(claim(ColoredRgbPsbt {
            psbt: moved.to_string(),
            allocations: allocations(),
        }));

        let wrong = [
            vec![],
            vec![RgbAllocation {
                vout: Some(0),
                ..allocations()[0].clone()
            }],
            vec![RgbAllocation {
                amount: 1_004,
                ..allocations()[0].clone()
            }],
            vec![RgbAllocation {
                asset_id: "rgb:other".to_string(),
                ..allocations()[0].clone()
            }],
            [allocations(), allocations()].concat(),
        ];
        for allocations in wrong {
            assert!(claim(ColoredRgbPsbt {
                psbt: color(&template),
                allocations,
            }));
        }

        // A claim with the wrong key or preimage is refused, not signed.
        let maker =
            Keypair::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&[9; 32]).unwrap());
        assert!(prepared
            .finalize_claim(colored(&template), &maker, &f.preimage)
            .is_err());
        let other = Preimage::from_vec([4; 32].to_vec()).unwrap();
        assert!(prepared
            .finalize_claim(colored(&template), &f.taker, &other)
            .is_err());
        assert!(prepared
            .finalize_refund(colored(&template), &f.taker)
            .is_err());
    }

    #[test]
    fn spends_are_only_the_takers() {
        let reverse = fixture(SwapType::ReverseSubmarine, 1_521);
        let submarine = fixture(SwapType::Submarine, 1_000);
        let new = |f: &Fixture, kind| {
            PreparedRgbSpend::new(
                kind,
                f.script.clone(),
                &dest(),
                BitcoinChain::BitcoinRegtest,
                (f.outpoint, f.utxo.clone()),
                RgbSpendFunding::CallerInputs,
                10_000,
            )
        };
        assert!(new(&reverse, SwapTxKind::Refund).is_err());
        assert!(new(&submarine, SwapTxKind::Claim).is_err());

        let mut short = reverse.utxo.clone();
        short.value = Amount::from_sat(1_520);
        assert!(PreparedRgbSpend::new(
            SwapTxKind::Claim,
            reverse.script.clone(),
            &dest(),
            BitcoinChain::BitcoinRegtest,
            (reverse.outpoint, short),
            RgbSpendFunding::CallerInputs,
            10_000,
        )
        .is_err());
        // A mainnet payout address on a regtest swap.
        assert!(PreparedRgbSpend::new(
            SwapTxKind::Claim,
            reverse.script.clone(),
            &dest_on(bitcoin::Network::Bitcoin),
            BitcoinChain::BitcoinRegtest,
            (reverse.outpoint, reverse.utxo.clone()),
            RgbSpendFunding::CallerInputs,
            10_000,
        )
        .is_err());
    }

    fn lock_of(f: &Fixture) -> RgbLock {
        let rgb = f.script.rgb.as_ref().unwrap();
        RgbLock {
            cooperative_refund: rgb.cooperative_refund.clone(),
            asset_id: rgb.contract_id.clone(),
            amount: rgb.amount,
            recipient_id: rgb.recipient_id.clone(),
            blinding: rgb.blinding.to_string(),
            htlc_sat: rgb.htlc_sat,
            claim_fee_rate: (f.script.swap_type == SwapType::ReverseSubmarine).then_some(5),
            script_pubkey: rgb.script_pubkey.to_hex_string(),
            transport_endpoints: rgb.transport_endpoints.clone(),
            min_confirmations: rgb.min_confirmations,
        }
    }

    fn swap_tree(f: &Fixture) -> crate::swaps::boltz::SwapTree {
        let leaf = |script: ScriptBuf| crate::swaps::boltz::Leaf {
            output: script.to_hex_string(),
            version: 0xc0,
        };
        crate::swaps::boltz::SwapTree {
            claim_leaf: leaf(f.script.claim_script()),
            refund_leaf: leaf(f.script.refund_script()),
        }
    }

    fn address(f: &Fixture) -> String {
        f.script
            .to_address(BitcoinChain::BitcoinRegtest)
            .unwrap()
            .to_string()
    }

    fn reverse_response(f: &Fixture) -> crate::swaps::boltz::CreateReverseResponse {
        crate::swaps::boltz::CreateReverseResponse {
            id: "rgb-reverse".to_string(),
            invoice: None,
            swap_tree: swap_tree(f),
            lockup_address: address(f),
            refund_public_key: f.script.sender_pubkey,
            timeout_block_height: 500,
            onchain_amount: 1_005,
            blinding_key: None,
            asset_id: None,
            fee_asset_id: None,
            swap_auth: None,
            rgb: Some(lock_of(f)),
        }
    }

    fn submarine_response(f: &Fixture) -> crate::swaps::boltz::CreateSubmarineResponse {
        crate::swaps::boltz::CreateSubmarineResponse {
            accept_zero_conf: false,
            address: address(f),
            bip21: String::new(),
            claim_public_key: f.script.receiver_pubkey,
            expected_amount: 1_005,
            id: "rgb-submarine".to_string(),
            referral_id: None,
            swap_tree: swap_tree(f),
            timeout_block_height: 500,
            blinding_key: None,
            asset_id: None,
            fee_asset_id: None,
            swap_auth: None,
            rgb: Some(lock_of(f)),
        }
    }

    fn invoice_for(preimage: &Preimage) -> String {
        use lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};
        let secp = Secp256k1::new();
        let key = SecretKey::from_slice(&[41; 32]).unwrap();
        InvoiceBuilder::new(Currency::Regtest)
            .description("rgb submarine".to_string())
            .payment_hash(preimage.sha256)
            .payment_secret(PaymentSecret([7; 32]))
            .duration_since_epoch(std::time::Duration::from_secs(1_726_000_000))
            .min_final_cltv_expiry_delta(144)
            .build_signed(|hash| secp.sign_ecdsa_recoverable(hash, &key))
            .unwrap()
            .to_string()
    }

    fn other_p2tr() -> ScriptBuf {
        ScriptBuf::new_p2tr_tweaked(TweakedPublicKey::dangerous_assume_tweaked(
            XOnlyPublicKey::from_slice(&[2; 32]).unwrap(),
        ))
    }

    #[test]
    fn the_maker_rgb_lock_json_round_trips() {
        let json = serde_json::json!({
            "assetId": CONTRACT,
            "amount": 969,
            "recipientId": RECIPIENT_VECTORS[1].1.replacen("wvout", "bcrt:wvout", 1),
            "blinding": "4611686018427387903",
            "htlcSat": 1521,
            "claimFeeRate": 5,
            "scriptPubkey": RECIPIENT_VECTORS[1].0,
            "transportEndpoints": ["rpcs://proxy.iriswallet.com/0.2/json-rpc"],
            "minConfirmations": 1
        });
        let lock: RgbLock = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&lock).unwrap(), json);
        let context = RgbHtlcContext::from_lock(&lock).unwrap();
        assert_eq!(context.blinding, (1 << 62) - 1);
        assert_eq!(context.recipient_network, RgbChainNet::BitcoinRegtest);

        // Submarine locks carry no claim fee rate, and omit the key.
        let mut submarine = json;
        submarine.as_object_mut().unwrap().remove("claimFeeRate");
        let lock: RgbLock = serde_json::from_value(submarine.clone()).unwrap();
        assert_eq!(lock.claim_fee_rate, None);
        assert_eq!(serde_json::to_value(&lock).unwrap(), submarine);
    }

    #[test]
    fn a_valid_rgb_reverse_response_validates_and_prices_in_sats() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        let response = reverse_response(&f);
        let our_key = PublicKey::new(f.taker.public_key());
        response
            .validate_rgb(
                &f.preimage,
                &our_key,
                BitcoinChain::BitcoinRegtest,
                CONTRACT,
            )
            .unwrap();
        // Dashes in a contract id are cosmetic.
        response
            .validate_rgb(
                &f.preimage,
                &our_key,
                BitcoinChain::BitcoinRegtest,
                &CONTRACT.replace('-', ""),
            )
            .unwrap();

        let script = BtcSwapScript::reverse_from_swap_resp(&response, our_key).unwrap();
        assert!(script.is_rgb());
        // The HTLC holds sats; the asset amount lives in the RGB context.
        assert_eq!(script.expected_amount, 1_521);
        assert_eq!(script.rgb.unwrap().amount, 1_005);
    }

    #[test]
    fn rgb_reverse_responses_are_refused_unless_every_binding_holds() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        let our_key = PublicKey::new(f.taker.public_key());
        let rejects =
            |what: &str, edit: &dyn Fn(&mut crate::swaps::boltz::CreateReverseResponse)| {
                let mut response = reverse_response(&f);
                edit(&mut response);
                assert!(
                    response
                        .validate_rgb(
                            &f.preimage,
                            &our_key,
                            BitcoinChain::BitcoinRegtest,
                            CONTRACT
                        )
                        .is_err(),
                    "accepted {what}"
                );
            };
        let lock = |r: &mut crate::swaps::boltz::CreateReverseResponse| -> RgbLock {
            r.rgb.clone().unwrap()
        };

        rejects("no RGB lock", &|r| r.rgb = None);
        rejects("another contract", &|r| {
            r.rgb = Some(RgbLock {
                asset_id: "rgb:another-contract".to_string(),
                ..lock(r)
            })
        });
        rejects("another script, with its own recipient", &|r| {
            let script = other_p2tr();
            r.rgb = Some(RgbLock {
                recipient_id: rgb_recipient_id(&script, RgbChainNet::BitcoinRegtest).unwrap(),
                script_pubkey: script.to_hex_string(),
                ..lock(r)
            })
        });
        rejects("a recipient for another script", &|r| {
            r.rgb = Some(RgbLock {
                recipient_id: rgb_recipient_id(&other_p2tr(), RgbChainNet::BitcoinRegtest).unwrap(),
                ..lock(r)
            })
        });
        rejects("a mainnet recipient", &|r| {
            let script = f.script.rgb.as_ref().unwrap().script_pubkey.clone();
            r.rgb = Some(RgbLock {
                recipient_id: rgb_recipient_id(&script, RgbChainNet::BitcoinMainnet).unwrap(),
                ..lock(r)
            })
        });
        rejects("an amount other than onchainAmount", &|r| {
            r.onchain_amount = 1_004
        });
        rejects("too few sats for the claim", &|r| {
            r.rgb = Some(RgbLock {
                htlc_sat: 1_520,
                ..lock(r)
            })
        });
        rejects("no claim fee rate", &|r| {
            r.rgb = Some(RgbLock {
                claim_fee_rate: None,
                ..lock(r)
            })
        });
        rejects("a non-numeric blinding", &|r| {
            r.rgb = Some(RgbLock {
                blinding: "0x2a".to_string(),
                ..lock(r)
            })
        });
        rejects("an HTTP transport", &|r| {
            r.rgb = Some(RgbLock {
                transport_endpoints: vec!["https://proxy.example/json-rpc".to_string()],
                ..lock(r)
            })
        });
        rejects("zero confirmations", &|r| {
            r.rgb = Some(RgbLock {
                min_confirmations: 0,
                ..lock(r)
            })
        });
        rejects("someone else's claim key", &|r| {
            r.refund_public_key = PublicKey::new(f.taker.public_key())
        });
    }

    #[test]
    fn plain_btc_validation_keeps_rgb_responses_out() {
        let f = fixture(SwapType::ReverseSubmarine, 1_521);
        let our_key = PublicKey::new(f.taker.public_key());
        let chain = crate::network::Chain::Bitcoin(BitcoinChain::BitcoinRegtest);
        let response = reverse_response(&f);
        // An RGB lock on a BTC swap, and USDT-RGB without the pinned contract.
        for currency in [None, Some(crate::network::Currency::Btc)] {
            assert!(response
                .validate_with_currency(&f.preimage, &our_key, chain, currency)
                .is_err());
        }
        assert!(response
            .validate_with_currency(
                &f.preimage,
                &our_key,
                chain,
                Some(crate::network::Currency::UsdtRgb)
            )
            .is_err());
        let mut plain = response;
        plain.rgb = None;
        plain.onchain_amount = 1_521;
        plain.validate(&f.preimage, &our_key, chain).unwrap();
    }

    #[test]
    fn a_valid_rgb_submarine_response_validates() {
        let f = fixture(SwapType::Submarine, 1_000);
        let our_key = PublicKey::new(f.taker.public_key());
        let invoice = invoice_for(&f.preimage);
        let response = submarine_response(&f);
        response
            .validate_rgb(&invoice, &our_key, BitcoinChain::BitcoinRegtest, CONTRACT)
            .unwrap();

        let mut short = response.clone();
        short.expected_amount = 1_004;
        assert!(short
            .validate_rgb(&invoice, &our_key, BitcoinChain::BitcoinRegtest, CONTRACT)
            .is_err());
        let other_invoice = invoice_for(&Preimage::from_vec([4; 32].to_vec()).unwrap());
        assert!(response
            .validate_rgb(
                &other_invoice,
                &our_key,
                BitcoinChain::BitcoinRegtest,
                CONTRACT
            )
            .is_err());
        assert!(response
            .validate_rgb(&invoice, &our_key, BitcoinChain::BitcoinSignet, CONTRACT)
            .is_err());
    }

    #[test]
    fn uncolored_spend_paths_refuse_rgb_htlcs() {
        let f = fixture(SwapType::Submarine, 1_000);
        let refund = BtcSwapTx {
            kind: SwapTxKind::Refund,
            swap_script: f.script.clone(),
            output_address: Address::from_str(&dest()).unwrap().assume_checked(),
            additional_outputs: Vec::new(),
            utxos: vec![(f.outpoint, f.utxo.clone())],
        };
        let fee = crate::util::fees::Fee::Absolute(500);
        let result = futures_util::FutureExt::now_or_never(refund.sign_refund(&f.taker, fee, None))
            .expect("refuses before any I/O");
        assert!(result.is_err());
        assert!(crate::swaps::SwapScriptCommon::partial_sign(
            &f.script,
            &f.taker,
            &"00".repeat(66),
            &"00".repeat(32),
        )
        .is_err());
    }
    fn cooperative_fixture(funding: RgbSpendFunding) -> (Fixture, PreparedRgbSpend) {
        let mut f = fixture(SwapType::Submarine, 5_000);
        f.script.rgb.as_mut().unwrap().cooperative_refund =
            Some(RGB_COOPERATIVE_REFUND_PROTOCOL.into());
        let prepared = PreparedRgbSpend::new_cooperative_refund(
            f.script.clone(),
            &dest(),
            BitcoinChain::BitcoinRegtest,
            (f.outpoint, f.utxo.clone()),
            funding,
            10_000,
        )
        .unwrap();
        (f, prepared)
    }

    fn maker_reply(session: &RgbCooperativeRefund) -> RgbCooperativeRefundResponse {
        let maker =
            Keypair::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&[9; 32]).unwrap());
        let (secret, nonce) = session.key_agg.nonce_gen(
            musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b()),
            convert_public_key(maker.public_key()),
            &session.message,
            Some(rng_32b()),
        );
        let aggregate = musig::AggregatedNonce::new(&[&nonce, &session.public_nonce]);
        let signer = musig::Session::new(&session.key_agg, aggregate, &session.message);
        let partial = signer.partial_sign(secret, &convert_keypair(&maker), &session.key_agg);
        RgbCooperativeRefundResponse {
            session_id: session.request.session_id.clone(),
            request_hash: session.request_hash.clone(),
            pub_nonce: nonce.to_string(),
            partial_signature: partial.to_string(),
        }
    }

    #[test]
    fn cooperative_refund_signs_the_colored_key_path_before_cltv() {
        let (f, prepared) = cooperative_fixture(RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 2 });
        assert_eq!(
            prepared.template.unsigned_tx.lock_time,
            bitcoin::absolute::LockTime::ZERO
        );
        assert!(prepared
            .finalize_refund(colored(&prepared.template().psbt), &f.taker)
            .is_err());
        let session = prepared
            .begin_cooperative_refund(colored(&prepared.template().psbt), &f.taker, "swap")
            .unwrap();
        let message = session.message;
        let response = maker_reply(&session);
        let tx = session
            .complete(response, &f.taker)
            .unwrap()
            .transaction
            .unwrap();
        assert_eq!(tx.input[0].witness.len(), 1);
        let signature =
            bitcoin::secp256k1::schnorr::Signature::from_slice(tx.input[0].witness.nth(0).unwrap())
                .unwrap();
        Secp256k1::new()
            .verify_schnorr(
                &signature,
                &Message::from_digest(message),
                &f.script
                    .taproot_spendinfo()
                    .unwrap()
                    .output_key()
                    .to_x_only_public_key(),
            )
            .unwrap();
        assert_eq!(
            tx.output[0].script_pubkey,
            ScriptBuf::new_op_return([0xab; 32])
        );
        assert_eq!(
            tx.compute_txid(),
            Psbt::from_str(&color(&prepared.template().psbt))
                .unwrap()
                .unsigned_tx
                .compute_txid()
        );
    }

    #[test]
    fn cooperative_refund_commits_to_wallet_fee_inputs_and_refuses_legacy_funding() {
        let (f, prepared) = cooperative_fixture(RgbSpendFunding::CallerInputs);
        let mut psbt = Psbt::from_str(&prepared.template().psbt).unwrap();
        psbt.unsigned_tx.input.push(TxIn {
            previous_output: OutPoint::new(Txid::from_byte_array([8; 32]), 0),
            sequence: RGB_SPEND_SEQUENCE,
            ..Default::default()
        });
        psbt.inputs.push(bitcoin::psbt::Input {
            witness_utxo: Some(TxOut {
                value: Amount::from_sat(1_000),
                script_pubkey: f.utxo.script_pubkey.clone(),
            }),
            ..Default::default()
        });
        let funded = prepared.fund(&psbt.to_string()).unwrap();
        let mut tampered = colored(&funded.template().psbt);
        let mut changed = Psbt::from_str(&tampered.psbt).unwrap();
        changed.inputs[1].witness_utxo.as_mut().unwrap().value = Amount::from_sat(2_000);
        tampered.psbt = changed.to_string();
        assert!(funded
            .begin_cooperative_refund(tampered, &f.taker, "swap")
            .is_err());
        let session = funded
            .begin_cooperative_refund(colored(&funded.template().psbt), &f.taker, "swap")
            .unwrap();
        let response = maker_reply(&session);
        let finalized = session.complete(response, &f.taker).unwrap();
        assert_eq!(finalized.swap_input_index, 0);
        assert_eq!(
            finalized.psbt.inputs[0]
                .final_script_witness
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert!(finalized.transaction.is_none());
        psbt.inputs[1].witness_utxo.as_mut().unwrap().script_pubkey =
            ScriptBuf::from_hex("76a914000000000000000000000000000000000000000088ac").unwrap();
        assert!(prepared.fund(&psbt.to_string()).is_err());
    }

    #[test]
    fn cooperative_refund_rejects_wrong_context_partial_key_and_script_path_intents() {
        let (f, prepared) = cooperative_fixture(RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 2 });
        for change in 0..3 {
            let session = prepared
                .begin_cooperative_refund(colored(&prepared.template().psbt), &f.taker, "swap")
                .unwrap();
            let mut response = maker_reply(&session);
            match change {
                0 => response.session_id = "00".repeat(32),
                1 => response.request_hash = "00".repeat(32),
                _ => response.partial_signature = "00".repeat(32),
            }
            assert!(session.complete(response, &f.taker).is_err());
        }
        let other = Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(&[10; 32]).unwrap(),
        );
        assert!(prepared
            .begin_cooperative_refund(colored(&prepared.template().psbt), &other, "swap")
            .is_err());
        let unilateral = prepare(
            &f,
            SwapTxKind::Refund,
            RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 2 },
        );
        assert!(unilateral
            .begin_cooperative_refund(colored(&unilateral.template().psbt), &f.taker, "swap")
            .is_err());
        let mut wrong = colored(&prepared.template().psbt);
        wrong.allocations[0].amount -= 1;
        assert!(prepared
            .begin_cooperative_refund(wrong, &f.taker, "swap")
            .is_err());
    }
}
