//! BTC Lightning -> L-USDT on Liquid reverse submarine swap.
//!
//! Run instructions and the wallet-funded PSET contract are documented in
//! `examples/README.md`.

mod lusdt_common;

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use kaleidorg_swap_sdk::elements::{Address as ElementsAddress, AddressParams};
use kaleidorg_swap_sdk::network::esplora::EsploraLiquidClient;
use kaleidorg_swap_sdk::network::{Chain, Currency, LiquidChain};
use kaleidorg_swap_sdk::swaps::boltz::CreateReverseRequest;
use kaleidorg_swap_sdk::swaps::{
    BtcLikeTransaction, ChainClient, FundedLiquidPset, LiquidPsetParams, SwapScript,
};
use kaleidorg_swap_sdk::util::secrets::Preimage;
use kaleidorg_swap_sdk::PublicKey;

use lusdt_common::{
    api_client, network, optional_env, optional_u64, required_env, sdk, swap_key, wait_for_file,
    wait_for_status,
};

/// Read `LUSDT_CLAIM_ADDRESS` and require that it encodes an address for
/// `chain`, so a testnet address can never be handed to a mainnet swap.
fn claim_address_for(chain: LiquidChain) -> Result<String> {
    let address = required_env("LUSDT_CLAIM_ADDRESS")?;
    let params: &'static AddressParams = chain.into();
    let parsed = ElementsAddress::parse_with_params(&address, params)
        .with_context(|| format!("LUSDT_CLAIM_ADDRESS is not a valid {chain:?} address"))?;
    if parsed.blinding_pubkey.is_none() {
        println!("note: LUSDT_CLAIM_ADDRESS is explicit; the L-USDT payout will be unblinded");
    }
    Ok(address)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (network, liquid_chain) = network()?;
    let api = api_client();
    let invoice_amount = optional_u64("INVOICE_AMOUNT_SATS", 100_000)?;
    // Pin the payout address to the selected chain up front. Waiting until the
    // claim is prepared would surface a typo or a wrong-network address only
    // after the hold invoice had already been paid.
    let claim_address = claim_address_for(liquid_chain)?;
    let esplora_url = optional_env("LIQUID_ESPLORA_URL", "http://localhost:4003/api");
    let template_path = PathBuf::from(optional_env(
        "LUSDT_PSET_TEMPLATE",
        "lusdt-reverse-claim-template.json",
    ));
    let funded_path = PathBuf::from(optional_env(
        "LUSDT_FUNDED_PSET",
        "lusdt-reverse-funded-pset.json",
    ));
    if funded_path.exists() {
        bail!(
            "{} already exists; remove the stale wallet response before starting",
            funded_path.display()
        );
    }

    let (swap_index, claim_key) = swap_key(network)?;
    let claim_public_key = PublicKey::new(claim_key.public_key());
    let preimage = Preimage::from_swap_key(&claim_key);

    let pairs = sdk(
        api.get_reverse_pairs().await,
        "fetch BTC/L-USDT reverse pair",
    )?;
    let pair = pairs
        .get_btc_to_lusdt_pair()
        .context("Maker does not advertise the BTC/L-USDT reverse pair")?;
    sdk(
        pair.limits.within(invoice_amount),
        "validate reverse amount against pair limits",
    )?;
    let expected_assets = sdk(
        pairs.expected_liquid_asset_context(Currency::Btc, Currency::LUsdt),
        "resolve pair asset ids",
    )?;

    let response = sdk(
        api.post_reverse_req(CreateReverseRequest {
            from: "BTC".to_owned(),
            to: "L-USDT".to_owned(),
            claim_public_key,
            invoice: None,
            invoice_amount: Some(invoice_amount),
            preimage_hash: Some(preimage.sha256),
            pair_hash: None,
            description: None,
            description_hash: None,
            address: None,
            address_signature: None,
            referral_id: None,
            webhook: None,
        })
        .await,
        "create BTC/L-USDT reverse swap",
    )?;

    // Never pay the invoice before this validation succeeds. It binds the
    // invoice payment hash, claim key, Liquid tree/address, L-USDT asset, and
    // L-BTC fee asset to the accepted pair card.
    sdk(
        response.validate_with_currency_and_asset_context(
            &preimage,
            &claim_public_key,
            Chain::Liquid(liquid_chain),
            Some(Currency::LUsdt),
            expected_assets,
        ),
        "validate reverse response",
    )?;
    let script = sdk(
        SwapScript::reverse_from_swap_resp(
            Chain::Liquid(liquid_chain),
            &response,
            claim_public_key,
        ),
        "reconstruct reverse claim script",
    )?;
    let invoice = response
        .invoice
        .as_deref()
        .context("reverse response omits its Lightning invoice")?;

    println!("swap id: {}", response.id);
    println!("swap key index: {swap_index}");
    println!("validated Lightning invoice:\n{invoice}");
    println!(
        "expected L-USDT payout: {} atomic units",
        response.onchain_amount
    );
    println!("Pay the invoice only after the validation above has succeeded.");

    wait_for_status(&api, &response.id, "transaction.confirmed").await?;

    let chain_client =
        ChainClient::new().with_liquid(EsploraLiquidClient::new(liquid_chain, &esplora_url, 30));
    let quoted_fee_cap = pair.fees.claim_estimate();
    let max_fee = optional_u64("MAX_CLAIM_FEE_SATS", quoted_fee_cap)?;
    let prepared = sdk(
        script
            .prepare_liquid_claim(LiquidPsetParams {
                output_address: claim_address,
                max_fee,
                quoted_fee_cap,
                swap_id: response.id.clone(),
                chain_client: &chain_client,
                boltz_api: &api,
                options: None,
            })
            .await,
        "prepare caller-funded L-USDT claim",
    )?;
    fs::write(
        &template_path,
        serde_json::to_vec_pretty(&prepared.template())?,
    )
    .with_context(|| format!("write {}", template_path.display()))?;
    println!(
        "wallet request written to {}; waiting for {}",
        template_path.display(),
        funded_path.display()
    );

    // The wallet adds an L-BTC fee input, optional L-BTC change, and an
    // explicit fee output; blinds outputs when needed; signs only its inputs;
    // and returns the funded PSET plus the payout output's unblinded secrets.
    wait_for_file(&funded_path).await?;
    let funded: FundedLiquidPset = serde_json::from_slice(
        &fs::read(&funded_path).with_context(|| format!("read {}", funded_path.display()))?,
    )
    .with_context(|| format!("decode {}", funded_path.display()))?;
    let claim = sdk(
        prepared.finalize_claim(funded, &claim_key, &preimage),
        "validate and finalize L-USDT claim",
    )?;
    let txid = sdk(
        chain_client
            .broadcast_tx(&BtcLikeTransaction::liquid(claim))
            .await,
        "broadcast L-USDT claim",
    )?;
    println!("claim broadcast: {txid}");

    wait_for_status(&api, &response.id, "invoice.settled").await?;
    println!("reverse swap complete: L-USDT was claimed to the wallet");
    Ok(())
}
