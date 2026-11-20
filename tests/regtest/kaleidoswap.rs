//! The high-level `KaleidoSwap` layer against the regtest maker: a submarine
//! swap whose Lightning payment fails is refunded from the chain.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bitcoin::key::rand::{thread_rng, RngCore};
use kaleidorg_swap_sdk::boltz::BOLTZ_REGTEST;
use kaleidorg_swap_sdk::network::{BitcoinChain, Chain, Network};
use kaleidorg_swap_sdk::swaps::kaleidoswap::{
    Amount, Asset, KaleidoSwap, KaleidoSwapOptions, MemoryStore, MnemonicSigner, Protocol,
    QuoteRequest, StartOptions, SwapErrorCode, SwapPhase, SwapRecord, SwapStore,
};
use kaleidorg_swap_sdk::util::{setup_logger, sleep};
use serial_test::serial;

use crate::utils;

const CHAIN: Chain = Chain::Bitcoin(BitcoinChain::BitcoinRegtest);
const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const INVOICE_SAT: u64 = 50_000;

/// A fresh instance over `store`. Every run gets its own passphrase so its
/// swap keys are new to the maker.
fn kaleidoswap(store: Arc<dyn SwapStore>, passphrase: &str) -> KaleidoSwap {
    let signer =
        MnemonicSigner::from_swap_mnemonic(MNEMONIC, Some(passphrase), Network::Regtest).unwrap();
    KaleidoSwap::new(KaleidoSwapOptions {
        network: Network::Regtest,
        store,
        signer: Arc::new(signer),
        maker_url: Some(BOLTZ_REGTEST.to_string()),
        // The layer's regtest defaults are the stack's Esplora instances.
        bitcoin_esplora_url: None,
        liquid_esplora_url: None,
        poll_interval: Some(Duration::from_secs(1)),
        fee_sat_per_vb: None,
        fee_policy: None,
        quote_ttl: None,
        maker_timeout: None,
        reverse_timeout_headroom: None,
    })
    .unwrap()
}

fn passphrase() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Mines a block every two seconds until dropped, so lockups and refunds
/// confirm while the swap is followed.
struct Miner(tokio::task::JoinHandle<()>);

impl Miner {
    fn start() -> Self {
        Self(tokio::spawn(async {
            loop {
                if let Err(e) = utils::mine_blocks(1).await {
                    log::warn!("Could not mine a block: {e}");
                }
                sleep(Duration::from_secs(2)).await;
            }
        }))
    }
}

impl Drop for Miner {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Starts an on-chain BTC -> Lightning submarine swap to an invoice that
/// cannot be paid, and funds its lockup with the exact amount asked for.
async fn start_and_fund(kaleido: &KaleidoSwap) -> SwapRecord {
    let invoice = utils::generate_cancelled_invoice_lnd(INVOICE_SAT)
        .await
        .unwrap();
    let quote = kaleido
        .quote(QuoteRequest {
            pay: Asset::BtcOnchain,
            receive: Asset::BtcLightning,
            amount: Amount::Invoice(invoice),
        })
        .await
        .unwrap()
        .quote()
        .unwrap();
    assert_eq!(quote.protocol, Protocol::BoltzSubmarine);
    assert_eq!(quote.to_amount, INVOICE_SAT);

    let refund_address = utils::generate_address(CHAIN).await.unwrap();
    let swap = kaleido
        .start(
            &quote,
            StartOptions {
                destination: None,
                refund_address: Some(refund_address),
            },
        )
        .await
        .unwrap();
    let record = swap.record();
    assert_eq!(record.phase, SwapPhase::AwaitingFunding);

    let action = record.action();
    let address = action.funding_address.unwrap();
    let amount = action.funding_amount.unwrap();
    log::info!("Funding swap {} with {amount} sat to {address}", record.id);
    utils::send_to_address(CHAIN, &address, amount)
        .await
        .unwrap();
    record
}

/// The refund the swap ends with is in a block, paying the refund address
/// from the node's own wallet.
async fn assert_refund_confirmed(record: &SwapRecord) {
    assert_eq!(record.phase, SwapPhase::Refunded);
    assert_eq!(record.raw_status, "invoice.failedToPay");
    assert!(record.state.pending_refund.is_none());
    let txid = record
        .state
        .refund_txid
        .as_deref()
        .expect("a refunded swap records its refund txid");
    let confirmations = utils::wallet_tx_confirmations(CHAIN, txid).await.unwrap();
    assert!(confirmations >= 1, "refund {txid} is not in a block");
}

/// The swap is followed in the process that started it: the payment fails,
/// the swap becomes refundable, the layer refunds it cooperatively and the
/// watch ends once the refund confirms, though the maker keeps reporting
/// `invoice.failedToPay`.
#[macros::async_test]
#[serial]
async fn kaleidoswap_submarine_failed_payment_refunds() {
    setup_logger();

    let store: Arc<dyn SwapStore> = Arc::new(MemoryStore::new());
    let kaleido = kaleidoswap(store.clone(), &passphrase());
    let started = start_and_fund(&kaleido).await;
    let _miner = Miner::start();

    let swap = kaleido.swap(&started.id).await.unwrap();
    let phases = Arc::new(Mutex::new(Vec::new()));
    let seen = phases.clone();
    let ended = tokio::time::timeout(
        Duration::from_secs(180),
        swap.watch(move |phase| seen.lock().unwrap().push(phase)),
    )
    .await
    .expect("the watch did not end");

    // A watch ends with an error on any end but `Settled`.
    let error = ended.unwrap_err();
    assert_eq!(error.code, SwapErrorCode::SwapFailed, "{error:?}");

    let phases = phases.lock().unwrap().clone();
    log::info!("Phases: {phases:?}");
    let tail: Vec<_> = phases
        .iter()
        .copied()
        .skip_while(|p| *p != SwapPhase::Refundable)
        .collect();
    assert_eq!(
        tail,
        vec![
            SwapPhase::Refundable,
            SwapPhase::Refunding,
            SwapPhase::Refunded
        ],
        "phases: {phases:?}"
    );

    let record = store.get(&started.id).await.unwrap().unwrap();
    assert_refund_confirmed(&record).await;

    // A terminal swap stays put: following it again ends at once.
    let again = kaleido.swap(&started.id).await.unwrap();
    assert_eq!(
        again.settled().await.unwrap_err().code,
        SwapErrorCode::SwapFailed
    );
}

/// The process that funded the swap stops before the payment fails. A new
/// instance over the same store and seed finds the swap on `resume`, refunds
/// it, and its handle follows the refund to `Refunded`.
#[macros::async_test]
#[serial]
async fn kaleidoswap_submarine_refund_resumes_in_fresh_process() {
    setup_logger();

    let store: Arc<dyn SwapStore> = Arc::new(MemoryStore::new());
    let passphrase = passphrase();
    let started = {
        let first = kaleidoswap(store.clone(), &passphrase);
        start_and_fund(&first).await
    };
    let _miner = Miner::start();

    // Nothing follows the swap while the maker sees the lockup and fails to
    // pay; the store still has it as the first process left it.
    let fresh = kaleidoswap(store.clone(), &passphrase);
    let mut refunding = None;
    for _ in 0..90 {
        let mut report = fresh.resume().await.unwrap();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        // A refund the chain source cannot build yet, because Esplora has not
        // indexed the lockup, is reported and left for the next pass.
        if let Some(failure) = report.errors.pop() {
            log::info!("Resume could not move the swap yet: {:?}", failure.error);
            sleep(Duration::from_secs(1)).await;
            continue;
        }
        assert!(report.settled.is_empty(), "{:?}", report.settled);
        if let Some(record) = report.refunded.pop() {
            refunding = Some(record);
            break;
        }
        let swap = report
            .resumed
            .pop()
            .expect("resume reports the swap in flight");
        let record = swap.record();
        log::info!(
            "Resumed swap at {} ({})",
            record.phase.as_str(),
            record.raw_status
        );
        if record.state.refund_txid.is_some() {
            assert_eq!(record.phase, SwapPhase::Refunding);
            assert_eq!(record.raw_status, "invoice.failedToPay");
            let ended = tokio::time::timeout(Duration::from_secs(120), swap.settled())
                .await
                .expect("settled() did not end");
            assert_eq!(ended.unwrap_err().code, SwapErrorCode::SwapFailed);
            refunding = store.get(&started.id).await.unwrap();
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let record = refunding.expect("the swap was never refunded");
    assert_refund_confirmed(&record).await;

    // Once refunded, there is nothing left to resume.
    let report = kaleidoswap(store, &passphrase).resume().await.unwrap();
    assert!(report.resumed.is_empty() && report.refunded.is_empty());
}
