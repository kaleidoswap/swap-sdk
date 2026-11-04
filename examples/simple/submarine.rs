//! Submarine swap with the high-level surface: on-chain BTC -> a Lightning invoice.
//!
//! You name what you pay and what you receive; `KaleidoSwap` prices the route,
//! checks the maker's response, persists the swap, and refunds on its own if
//! the maker cannot pay. `examples/advanced/submarine.rs` is the same swap
//! step by step on the engine.
//!
//! ```text
//! KALEIDO_SWAP_MNEMONIC="…12 words…" BOLT11_INVOICE="lntbs…" REFUND_ADDRESS="tb1…" \
//!   cargo run --example simple_submarine
//! ```

// `FileStore` keeps the swaps on disk, so this is a native program. The wasm
// build only compiles it, as `cargo test` does for every example; in a browser
// an application gives `KaleidoSwap` a store of its own.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use {
    anyhow::{Context, Result},
    kaleidorg_swap_sdk::kaleidoswap::{
        Amount, Asset, FileStore, KaleidoSwap, KaleidoSwapOptions, MnemonicSigner, QuoteRequest,
        StartOptions,
    },
    kaleidorg_swap_sdk::network::Network,
    std::{env, path::PathBuf, sync::Arc},
};

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn main() {}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let network = Network::Signet;
    let mnemonic =
        env::var("KALEIDO_SWAP_MNEMONIC").context("KALEIDO_SWAP_MNEMONIC must be set")?;
    let invoice = env::var("BOLT11_INVOICE").context("BOLT11_INVOICE must be set")?;
    let refund_address = env::var("REFUND_ADDRESS").context("REFUND_ADDRESS must be set")?;

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
            pay: Asset::BtcOnchain,
            receive: Asset::BtcLightning,
            amount: Amount::Invoice(invoice), // the invoice fixes the amount
        })
        .await?
        .quote()
        .map_err(|refusal| anyhow::anyhow!("{:?}: {}", refusal.reason, refusal.message))?;
    println!(
        "{} sat on chain pays a {} sat invoice (maker fee {}, miner fee {})",
        quote.from_amount, quote.to_amount, quote.fees.maker, quote.fees.miner
    );

    let swap = kaleido
        .start(
            &quote,
            StartOptions {
                destination: None,
                refund_address: Some(refund_address),
            },
        )
        .await?;
    let action = swap.action();
    println!(
        "\nFund exactly {} sat to:\n  {}\n",
        action.funding_amount.unwrap_or_default(),
        action.funding_address.unwrap_or_default()
    );

    // `watch` also ends on `refunded` and `failed`, as an error; only
    // `settled` is a success.
    swap.watch(|phase| println!("  {}", phase.as_str()))
        .await
        .with_context(|| format!("swap {} ended without paying the invoice", swap.id()))?;
    println!("Done: the invoice was paid ({})", swap.id());
    Ok(())
}
