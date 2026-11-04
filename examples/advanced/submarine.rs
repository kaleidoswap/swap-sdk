//! Submarine swap with the engine: on-chain BTC -> a Lightning invoice.
//!
//! You lock bitcoin, the maker pays the invoice and claims the lockup. Every
//! step is the caller's: pin the pair card, create, check the response against
//! our own refund key, persist, and refund if the maker cannot pay. The SDK
//! owns no wallet, so it prints what to fund. `examples/simple/submarine.rs`
//! is the same swap through `KaleidoSwap`.
//!
//! ```text
//! KALEIDO_NETWORK=signet KALEIDO_MAKER_URL=https://maker.signet.kaleidoswap.com/v2 \
//! KALEIDO_SWAP_MNEMONIC="…12 words…" KALEIDO_SWAP_INDEX=0 \
//! BOLT11_INVOICE="lntbs…" REFUND_ADDRESS="tb1…" \
//!   cargo run --example advanced_submarine
//! ```

mod common;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use kaleidorg_swap_sdk::fees::Fee;
use kaleidorg_swap_sdk::network::esplora::EsploraBitcoinClient;
use kaleidorg_swap_sdk::network::{BitcoinChain, Chain, Currency};
use kaleidorg_swap_sdk::swaps::boltz::CreateSubmarineRequest;
use kaleidorg_swap_sdk::swaps::{
    ChainClient, SwapScript, SwapTransactionParams, TransactionOptions,
};
use kaleidorg_swap_sdk::PublicKey;

use common::{api_client, network, optional_env, required_env, sdk, swap_key, wait_for_any};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (network, _) = network()?;
    let chain = BitcoinChain::from(network);
    let api = api_client();
    let invoice = required_env("BOLT11_INVOICE")?;
    let refund_address = required_env("REFUND_ADDRESS")?;
    let esplora_url = optional_env("BITCOIN_ESPLORA_URL", "http://localhost:3002");

    // One BIP85 child per swap: reusing an index reuses refund material.
    let (swap_index, refund_key) = swap_key(network)?;
    let refund_public_key = PublicKey::new(refund_key.public_key());

    // Passing the card's hash back is what holds the maker to these terms.
    let pairs = sdk(api.get_submarine_pairs().await, "fetch submarine pairs")?;
    let card = pairs
        .get("BTC", "BTC")
        .context("the maker does not advertise the BTC -> BTC submarine pair")?
        .clone();

    let response = sdk(
        api.post_swap_req(&CreateSubmarineRequest {
            from: "BTC".to_owned(),
            to: "BTC".to_owned(),
            invoice: invoice.clone(),
            refund_public_key,
            pair_hash: Some(card.hash.clone()),
            referral_id: None,
            webhook: None,
        })
        .await,
        "create BTC submarine swap",
    )?;

    // Rebuilds the lockup against our own refund key and the invoice we gave:
    // a tree we could not refund has to fail here, before funding.
    sdk(
        response.validate_with_currency_and_asset_context(
            &invoice,
            &refund_public_key,
            Chain::Bitcoin(chain),
            Some(Currency::Btc),
            None,
        ),
        "validate submarine response",
    )?;

    // swap_auth rides in the response, issued once; a cooperative refund needs
    // it. Persist before funding, and never log the response.
    let state = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/swaps/advanced");
    fs::create_dir_all(&state)?;
    let saved = state.join(format!("{}.json", response.id));
    fs::write(
        &saved,
        serde_json::to_vec_pretty(&serde_json::json!({
            "swapIndex": swap_index,
            "response": response,
        }))?,
    )?;

    println!("swap id:    {}", response.id);
    println!("swap index: {swap_index}  (never reuse it)");
    println!("saved to:   {}", saved.display());
    println!(
        "\nFund exactly {} sat to:\n  {}\n",
        response.expected_amount, response.address
    );
    if !response.accept_zero_conf {
        println!("The maker waits for a confirmation before paying the invoice.");
    }

    let outcome = wait_for_any(
        &api,
        &response.id,
        &[
            "transaction.claimed",
            "invoice.failedToPay",
            "transaction.lockupFailed",
            "swap.expired",
        ],
    )
    .await?;
    if outcome == "transaction.claimed" {
        println!("\nDone: the invoice was paid and the lockup claimed.");
        return Ok(());
    }
    println!("\n{outcome}: refunding the lockup to {refund_address}");

    // The maker could not pay: take the lockup back. Cooperative first, the
    // maker co-signing so there is no timelock to wait out.
    let chain_client =
        ChainClient::new().with_bitcoin(EsploraBitcoinClient::new(chain, &esplora_url, 30));
    let script = sdk(
        SwapScript::submarine_from_swap_resp(Chain::Bitcoin(chain), &response, refund_public_key),
        "rebuild the submarine script",
    )?;
    let mut options = TransactionOptions::default().with_cooperative(true);
    if let Some(auth) = response.swap_auth.clone() {
        options = options.with_swap_auth(auth);
    }
    let refund = sdk(
        script
            .construct_refund(SwapTransactionParams {
                keys: refund_key,
                output_address: refund_address.clone(),
                fee: Fee::Relative(2.0),
                swap_id: response.id.clone(),
                chain_client: &chain_client,
                boltz_api: &api,
                options: Some(options),
            })
            .await,
        // Without the maker's signature the script path opens at the
        // lockup's timeout: retry then with `with_cooperative(false)`.
        "build the cooperative refund",
    )?;
    let txid = sdk(chain_client.broadcast_tx(&refund).await, "broadcast refund")?;
    println!("\nrefund broadcast: {txid} -> {refund_address}");
    Ok(())
}
