//! L-USDT on Liquid -> BTC Lightning submarine swap.
//!
//! Run instructions and required environment variables are documented in
//! `examples/README.md`.

mod lusdt_common;

use anyhow::{Context, Result};
use kaleidorg_swap_sdk::network::{Chain, Currency};
use kaleidorg_swap_sdk::swaps::boltz::CreateSubmarineRequest;
use kaleidorg_swap_sdk::swaps::SwapScript;
use kaleidorg_swap_sdk::{LiquidAssetContext, PublicKey};

use lusdt_common::{api_client, network, required_env, sdk, swap_key, wait_for_status};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (network, liquid_chain) = network()?;
    let api = api_client();
    let invoice = required_env("BOLT11_INVOICE")?;
    let (swap_index, refund_key) = swap_key(network)?;
    let refund_public_key = PublicKey::new(refund_key.public_key());

    // Always obtain the pair hash from the pair card immediately before
    // creating the swap. The asset ids on the card are read too, but only to
    // cross-check the pinned asset below: the maker does not get to say which
    // asset is L-USDT.
    let pairs = sdk(
        api.get_submarine_pairs().await,
        "fetch L-USDT submarine pair",
    )?;
    let pair = pairs
        .get_lusdt_to_btc_pair()
        .context("Maker does not advertise the L-USDT/BTC submarine pair")?;
    // The pair card is the maker's claim about which asset is L-USDT; the
    // trusted asset is the canonical USDt on mainnet, or `LUSDT_ASSET_ID`
    // elsewhere, and the card is only cross-checked against it.
    let pair_card = sdk(
        pairs.expected_liquid_asset_context(Currency::LUsdt, Currency::Btc),
        "read pair asset ids",
    )?;
    let expected_assets = sdk(
        LiquidAssetContext::trusted_lusdt(
            liquid_chain,
            std::env::var("LUSDT_ASSET_ID").ok().as_deref(),
        )
        .and_then(|trusted| trusted.cross_check_pair_card(pair_card)),
        "pin the L-USDT asset",
    )?;

    let response = sdk(
        api.post_swap_req(&CreateSubmarineRequest {
            from: "L-USDT".to_owned(),
            to: "BTC".to_owned(),
            invoice: invoice.clone(),
            refund_public_key,
            pair_hash: Some(pair.hash),
            referral_id: None,
            webhook: None,
        })
        .await,
        "create L-USDT submarine swap",
    )?;

    // This must succeed before the wallet funds anything. It binds the
    // invoice hash, refund key, Liquid tree/address, L-USDT asset, and L-BTC
    // fee asset to the pair card accepted above.
    sdk(
        response.validate_with_currency_and_asset_context(
            &invoice,
            &refund_public_key,
            Chain::Liquid(liquid_chain),
            Some(Currency::LUsdt),
            Some(expected_assets),
        ),
        "validate submarine response",
    )?;
    let _refund_script = sdk(
        SwapScript::submarine_from_swap_resp(
            Chain::Liquid(liquid_chain),
            &response,
            refund_public_key,
        ),
        "reconstruct refund script",
    )?;

    println!("swap id: {}", response.id);
    println!("swap key index: {swap_index}");
    println!("lockup address: {}", response.address);
    println!("L-USDT atomic units to send: {}", response.expected_amount);
    println!(
        "L-USDT asset id: {}",
        response
            .asset_id
            .as_deref()
            .context("response omits assetId")?
    );
    println!(
        "L-BTC fee asset id: {}",
        response
            .fee_asset_id
            .as_deref()
            .context("response omits feeAssetId")?
    );
    println!(
        "\nFund the lockup with exactly {} units of the advertised L-USDT asset.",
        response.expected_amount
    );
    println!("Your Liquid wallet must pay the funding transaction fee in L-BTC.");
    println!("Do not reuse KALEIDO_SWAP_INDEX for another swap.");

    wait_for_status(&api, &response.id, "transaction.claimed").await?;
    println!("submarine swap complete: the BTC Lightning invoice was paid");
    Ok(())
}
