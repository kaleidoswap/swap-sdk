//! The colored RGB HTLC spends against a real bitcoind: the claim of a
//! reverse lock, and the refund of a submarine lock with and without a fee
//! input. rgb-lib's coloring is stood in for by writing a commitment into
//! output 0, which is all `psbt_op_prepare` changes in the transaction; the
//! RGB side of these spends is covered by the maker's rgb-lib regtest e2e.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]

use bitcoin::absolute::LockTime;
use bitcoin::key::rand::thread_rng;
use bitcoin::key::{Keypair, PublicKey};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Amount, Psbt, ScriptBuf, Transaction, TxOut};
use bitcoind::bitcoincore_rpc::json::AddressType;
use bitcoind::bitcoincore_rpc::RpcApi;
use kaleidorg_swap_sdk::boltz::SwapType;
use kaleidorg_swap_sdk::network::BitcoinChain;
use kaleidorg_swap_sdk::rgb::{
    claim_leaf_witness_weight, taker_claim_htlc_sat, RgbFeeInput, RgbHtlcSpend, RgbLock,
    MIN_HTLC_OUTPUT_SAT,
};
use kaleidorg_swap_sdk::util::secrets::Preimage;
use kaleidorg_swap_sdk::BtcSwapScript;

mod test_framework;
use test_framework::BtcTestFramework;

const CLAIM_FEE_RATE: u64 = 2;

/// `OP_SIZE <32> OP_EQUALVERIFY OP_HASH160 <20> OP_EQUALVERIFY <32> OP_CHECKSIG`.
const REVERSE_CLAIM_LEAF_LEN: u64 = 61;

fn keypair() -> Keypair {
    Keypair::new(&Secp256k1::new(), &mut thread_rng())
}

fn swap_script(
    swap_type: SwapType,
    preimage: &Preimage,
    claim: &Keypair,
    refund: &Keypair,
    timeout: u32,
) -> BtcSwapScript {
    BtcSwapScript {
        swap_type,
        side: None,
        funding_addrs: None,
        hashlock: preimage.hash160,
        receiver_pubkey: PublicKey::new(claim.public_key()),
        locktime: LockTime::from_height(timeout).unwrap(),
        sender_pubkey: PublicKey::new(refund.public_key()),
        expected_amount: 1_000_000,
    }
}

fn lock_for(address: &Address, htlc_sat: u64, claim_fee_rate: Option<u64>) -> RgbLock {
    RgbLock {
        asset_id: "rgb:regtest".into(),
        amount: 1_000_000,
        recipient_id: "bcrt:wvout:htlc".into(),
        blinding: 1,
        htlc_sat,
        claim_fee_rate,
        script_pubkey: address.script_pubkey().to_hex_string(),
        transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".into()],
        min_confirmations: 1,
    }
}

/// Fund `address` with `sat` and return the funding transaction.
fn fund(framework: &BtcTestFramework, address: &Address, sat: u64) -> Transaction {
    let txid = framework.send_coins(address, Amount::from_sat(sat));
    framework.generate_blocks(1);
    framework.as_ref().get_raw_transaction(&txid, None).unwrap()
}

/// What rgb-lib's `psbt_op_prepare` does to the transaction.
fn color(mut psbt: Psbt) -> Psbt {
    psbt.unsigned_tx.output[0].script_pubkey = ScriptBuf::new_op_return([0xab; 32]);
    psbt
}

fn colored_dest(framework: &BtcTestFramework) -> ScriptBuf {
    framework
        .get_test_wallet()
        .get_new_address(None, Some(AddressType::Bech32m))
        .unwrap()
        .assume_checked()
        .script_pubkey()
}

fn height(framework: &BtcTestFramework) -> u32 {
    framework.as_ref().get_block_count().unwrap() as u32
}

#[test]
fn a_colored_claim_of_a_reverse_lock_is_valid_without_other_inputs() {
    let framework = BtcTestFramework::init();
    let (taker, maker, preimage) = (keypair(), keypair(), Preimage::random());
    let timeout = height(&framework) + 100;
    let script = swap_script(
        SwapType::ReverseSubmarine,
        &preimage,
        &taker,
        &maker,
        timeout,
    );
    let address = script.to_address(BitcoinChain::BitcoinRegtest).unwrap();
    let htlc_sat = taker_claim_htlc_sat(
        claim_leaf_witness_weight(REVERSE_CLAIM_LEAF_LEN),
        CLAIM_FEE_RATE,
    )
    .unwrap();
    let lock = lock_for(&address, htlc_sat, Some(CLAIM_FEE_RATE));
    let lock_tx = fund(&framework, &address, htlc_sat);
    let htlc = lock.find_htlc_output(&lock_tx).unwrap();

    let spend = RgbHtlcSpend::claim(&script, &lock, htlc, colored_dest(&framework), None).unwrap();
    let claim = spend
        .sign_colored_tx(&color(spend.psbt().unwrap()), &taker, Some(&preimage))
        .unwrap();
    assert_eq!(claim.input.len(), 1);
    assert_eq!(claim.output[1].value.to_sat(), MIN_HTLC_OUTPUT_SAT);
    assert!(spend.fee_sat() >= claim.vsize() as u64 * CLAIM_FEE_RATE);

    let txid = framework.as_ref().send_raw_transaction(&claim).unwrap();
    framework.generate_blocks(1);
    let confirmed = framework
        .as_ref()
        .get_raw_transaction_info(&txid, None)
        .unwrap();
    assert!(confirmed.confirmations.unwrap_or(0) >= 1);
}

#[test]
fn a_colored_refund_of_a_submarine_lock_is_valid_after_the_timeout() {
    let framework = BtcTestFramework::init();
    let (taker, maker, preimage) = (keypair(), keypair(), Preimage::random());
    let timeout = height(&framework) + 3;
    let script = swap_script(SwapType::Submarine, &preimage, &maker, &taker, timeout);
    let address = script.to_address(BitcoinChain::BitcoinRegtest).unwrap();
    let lock = lock_for(&address, 1_000, None);

    // HTLC alone, at 1 sat/vB.
    let lock_tx = fund(&framework, &address, 1_000);
    let htlc = lock.find_htlc_output(&lock_tx).unwrap();
    let spend =
        RgbHtlcSpend::refund(&script, &lock, htlc, colored_dest(&framework), None, 1).unwrap();
    let refund = spend
        .sign_colored_tx(&color(spend.psbt().unwrap()), &taker, None)
        .unwrap();
    assert!(
        framework.as_ref().send_raw_transaction(&refund).is_err(),
        "not final before the timeout"
    );

    // With a fee input from the wallet, which signs its own input.
    let lock_tx = fund(&framework, &address, 1_000);
    let htlc = lock.find_htlc_output(&lock_tx).unwrap();
    let wallet = framework.get_test_wallet();
    let fee_address = wallet
        .get_new_address(None, Some(AddressType::Bech32m))
        .unwrap()
        .assume_checked();
    let fee_tx = fund(&framework, &fee_address, 20_000);
    let (fee_vout, fee_txout) = fee_tx
        .output
        .iter()
        .enumerate()
        .find(|(_, o)| o.script_pubkey == fee_address.script_pubkey())
        .map(|(vout, o)| (vout as u32, o.clone()))
        .unwrap();
    let fee_input = RgbFeeInput {
        outpoint: bitcoin::OutPoint::new(fee_tx.compute_txid(), fee_vout),
        txout: TxOut {
            value: fee_txout.value,
            script_pubkey: fee_txout.script_pubkey,
        },
        change_script: colored_dest(&framework),
    };
    let with_fee = RgbHtlcSpend::refund(
        &script,
        &lock,
        htlc,
        colored_dest(&framework),
        Some(fee_input),
        5,
    )
    .unwrap();
    let half_signed = with_fee
        .sign_colored(&color(with_fee.psbt().unwrap()), &taker, None)
        .unwrap();
    let processed = wallet
        .wallet_process_psbt(&half_signed.to_string(), Some(true), None, None)
        .unwrap();
    let finalized = wallet.finalize_psbt(&processed.psbt, Some(true)).unwrap();
    assert!(finalized.complete, "the wallet finalizes its fee input");
    let raw = finalized.hex.unwrap();

    while height(&framework) < timeout {
        framework.generate_blocks(1);
    }
    framework.as_ref().send_raw_transaction(&refund).unwrap();
    framework.as_ref().send_raw_transaction(&raw).unwrap();
    framework.generate_blocks(1);
    let tx: Transaction = bitcoin::consensus::deserialize(&raw).unwrap();
    assert_eq!(tx.output[1].value.to_sat(), 1_000, "every HTLC sat colored");
    assert!(framework
        .as_ref()
        .get_raw_transaction_info(&tx.compute_txid(), None)
        .unwrap()
        .confirmations
        .is_some());
}
