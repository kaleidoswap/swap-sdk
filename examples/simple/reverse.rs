//! Reverse swap with the high-level surface: a Lightning payment -> L-BTC on Liquid.
//!
//! You name what you pay and what you receive; `KaleidoSwap` prices the route,
//! checks the maker's response, persists the swap, and claims the L-BTC to your
//! address once the maker's lockup confirms. `examples/advanced/reverse.rs` is
//! a reverse swap step by step on the engine, to on-chain BTC.
//!
//! ```text
//! KALEIDO_SWAP_MNEMONIC="…12 words…" LIQUID_CLAIM_ADDRESS="tlq1…" \
//!   cargo run --example simple_reverse
//! ```

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use kaleidorg_swap_sdk::kaleidoswap::{
    Amount, Asset, FileStore, KaleidoSwap, KaleidoSwapOptions, MnemonicSigner, QuoteRequest,
    StartOptions,
};
use kaleidorg_swap_sdk::network::Network;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let network = Network::Signet;
    let mnemonic =
        env::var("KALEIDO_SWAP_MNEMONIC").context("KALEIDO_SWAP_MNEMONIC must be set")?;
    let destination =
        env::var("LIQUID_CLAIM_ADDRESS").context("LIQUID_CLAIM_ADDRESS must be set")?;

    // The store keeps every swap until it ends: claim and refund material, and
    // the maker's swap_auth. It is gitignored; treat it as secret.
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/swaps/simple");
    let kaleido = KaleidoSwap::new(KaleidoSwapOptions {
        network,
        store: Arc::new(FileStore::new(store)),
        signer: Arc::new(MnemonicSigner::from_swap_mnemonic(
            &mnemonic, None, network,
        )?),
        maker_url: None,
        bitcoin_esplora_url: None,
        liquid_esplora_url: None,
        poll_interval: None,
        fee_sat_per_vb: None,
        quote_ttl: None,
        maker_timeout: None,
    })?;
    kaleido.resume().await?; // claims or refunds anything a previous run left behind

    let quote = kaleido
        .quote(QuoteRequest {
            pay: Asset::BtcLightning,
            receive: Asset::BtcLiquid,
            // What you pay; `Amount::Receive` fixes what lands instead.
            amount: Amount::Pay(50_000),
        })
        .await?
        .quote()
        .map_err(|refusal| anyhow::anyhow!("{:?}: {}", refusal.reason, refusal.message))?;
    println!(
        "pay {} sat over Lightning, receive {} sat of L-BTC (maker fee {}, miner fee {})",
        quote.from_amount, quote.to_amount, quote.fees.maker, quote.fees.miner
    );

    let swap = kaleido
        .start(
            &quote,
            StartOptions {
                destination: Some(destination),
                refund_address: None,
            },
        )
        .await?;
    println!(
        "\nPay this invoice ({} sat):\n\n{}\n",
        quote.from_amount,
        swap.action().invoice.unwrap_or_default()
    );

    // `watch` also ends on `refunded` and `failed`, as an error; only
    // `settled` is a success.
    swap.watch(|phase| println!("  {}", phase.as_str()))
        .await
        .with_context(|| format!("swap {} ended without settling", swap.id()))?;
    println!(
        "Done: ~{} sat of L-BTC reached the claim address ({})",
        quote.to_amount,
        swap.id()
    );
    Ok(())
}
