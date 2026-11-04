//! Reverse swap with the engine: a Lightning payment -> on-chain BTC.
//!
//! The maker locks bitcoin once its hold invoice is paid, and we claim it with
//! the preimage. Every step is the caller's: pin the pair card, create, check
//! the response against our own claim key and preimage, check the timeout
//! against our own chain tip, persist, then claim on confirmation. The only
//! manual step is paying the printed invoice. `examples/simple/reverse.rs` is
//! a reverse swap through `KaleidoSwap`.
//!
//! ```text
//! KALEIDO_NETWORK=signet KALEIDO_MAKER_URL=https://maker.signet.kaleidoswap.com/v2 \
//! BITCOIN_ESPLORA_URL=https://esplora.signet.kaleidoswap.com \
//! KALEIDO_SWAP_MNEMONIC="…12 words…" KALEIDO_SWAP_INDEX=0 \
//! BTC_CLAIM_ADDRESS="tb1…" \
//!   cargo run --example advanced_reverse
//! ```

mod common;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use kaleidorg_swap_sdk::fees::Fee;
use kaleidorg_swap_sdk::network::esplora::EsploraBitcoinClient;
use kaleidorg_swap_sdk::network::{BitcoinChain, BitcoinClient, Chain, Currency};
use kaleidorg_swap_sdk::swaps::boltz::CreateReverseRequest;
use kaleidorg_swap_sdk::swaps::{
    ChainClient, SwapScript, SwapTransactionParams, TransactionOptions,
};
use kaleidorg_swap_sdk::util::secrets::Preimage;
use kaleidorg_swap_sdk::PublicKey;

use common::{
    api_client, network, optional_env, optional_u64, required_env, sdk, swap_key, wait_for_status,
    write_secret,
};

/// Blocks the lockup must have left before its timeout when we pay: room to
/// confirm, then the claim path's own safety margin.
const MIN_TIMEOUT_HEADROOM_BLOCKS: u32 = 12;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (network, _) = network()?;
    let chain = BitcoinChain::from(network);
    let api = api_client();
    let amount = optional_u64("INVOICE_AMOUNT_SATS", 50_000)?;
    let esplora_url = optional_env("BITCOIN_ESPLORA_URL", "http://localhost:3002");

    // Pin the payout address to this network before anything is paid: a
    // wrong-network address would surface only at the claim, after the payment.
    let claim_address = required_env("BTC_CLAIM_ADDRESS")?;
    claim_address
        .parse::<kaleidorg_swap_sdk::bitcoin::Address<_>>()
        .context("BTC_CLAIM_ADDRESS is not a Bitcoin address")?
        .require_network(chain.into())
        .with_context(|| format!("BTC_CLAIM_ADDRESS is not a {chain:?} address"))?;

    // One BIP85 child per swap; the preimage derives from the same key.
    let (swap_index, claim_key) = swap_key(network)?;
    let claim_public_key = PublicKey::new(claim_key.public_key());
    let preimage = Preimage::from_swap_key(&claim_key);

    let pairs = sdk(api.get_reverse_pairs().await, "fetch reverse pairs")?;
    let card = pairs
        .get("BTC", "BTC")
        .context("the maker does not advertise the BTC -> BTC reverse pair")?
        .clone();
    sdk(
        card.limits.within(amount),
        "check the amount against the pair limits",
    )?;

    let request = CreateReverseRequest {
        from: "BTC".to_owned(),
        to: "BTC".to_owned(),
        claim_public_key,
        invoice: None,
        invoice_amount: Some(amount),
        preimage_hash: Some(preimage.sha256),
        pair_hash: Some(card.hash.clone()),
        description: None,
        description_hash: None,
        address: None,
        address_signature: None,
        referral_id: None,
        webhook: None,
    };
    let response = sdk(
        api.post_reverse_req(request.clone()).await,
        "create BTC reverse swap",
    )?;

    // Never pay before these pass. The invoice must carry the amount asked for
    // and our own payment hash; the lockup tree must be one our key can claim.
    sdk(
        response.validate_amounts(&request, Some(&card)),
        "check the invoice and payout amounts",
    )?;
    sdk(
        response.validate_with_source_and_asset_context(
            &preimage,
            &claim_public_key,
            Chain::Bitcoin(chain),
            Chain::Bitcoin(chain),
            Some(Currency::Btc),
            None,
        ),
        "validate reverse response",
    )?;

    // The maker refunds its lockup at the timeout. Check it against our own
    // chain tip, never the maker's: a nearly mature lockup is not one to pay.
    let esplora = EsploraBitcoinClient::new(chain, &esplora_url, 30);
    let tip = sdk(esplora.get_tip_height().await, "read the chain tip")?;
    sdk(
        response.validate_timeout_headroom(tip, MIN_TIMEOUT_HEADROOM_BLOCKS),
        "check the lockup timeout",
    )?;

    // swap_auth rides in the response, issued once. Persist before paying.
    let state = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/swaps/advanced");
    fs::create_dir_all(&state)?;
    let saved = state.join(format!("{}.json", response.id));
    write_secret(
        &saved,
        &serde_json::to_vec_pretty(&serde_json::json!({
            "swapIndex": swap_index,
            "response": response,
        }))?,
    )?;
    let invoice = response
        .invoice
        .as_deref()
        .context("the reverse response carries no invoice")?;

    println!("swap id:    {}", response.id);
    println!("swap index: {swap_index}  (never reuse it)");
    println!("saved to:   {}", saved.display());
    println!("\nPay this invoice ({amount} sat):\n\n{invoice}\n");
    println!("Waiting for the maker's lockup to confirm.");

    wait_for_status(&api, &response.id, "transaction.confirmed").await?;

    // The claim reveals the preimage, so `construct_claim` checks the lockup
    // on our chain client first: confirmed, and clear of the timeout. The
    // maker co-signs a key-path claim; the script path is the fallback.
    let chain_client = ChainClient::new().with_bitcoin(esplora);
    let script = sdk(
        SwapScript::reverse_from_swap_resp(Chain::Bitcoin(chain), &response, claim_public_key),
        "rebuild the reverse script",
    )?;
    let params = |cooperative: bool| {
        let mut options = TransactionOptions::default().with_cooperative(cooperative);
        if let Some(auth) = response.swap_auth.clone() {
            options = options.with_swap_auth(auth);
        }
        SwapTransactionParams {
            keys: claim_key,
            output_address: claim_address.clone(),
            fee: Fee::Relative(2.0),
            swap_id: response.id.clone(),
            chain_client: &chain_client,
            boltz_api: &api,
            options: Some(options),
        }
    };
    let claim = match script.construct_claim(&preimage, params(true)).await {
        Ok(tx) => tx,
        Err(error) => {
            println!("cooperative claim unavailable ({error}); claiming by script path");
            sdk(
                script.construct_claim(&preimage, params(false)).await,
                "build the claim",
            )?
        }
    };
    let txid = sdk(chain_client.broadcast_tx(&claim).await, "broadcast claim")?;
    println!("\nclaim broadcast: {txid}");

    // The maker settles the hold invoice once it sees the preimage on chain.
    wait_for_status(&api, &response.id, "invoice.settled").await?;
    println!(
        "\nDone: {} sat claimed to {claim_address}",
        response.onchain_amount
    );
    Ok(())
}
