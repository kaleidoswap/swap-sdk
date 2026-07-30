//! Live smoke against the KaleidoSwap **signet** maker (run manually:
//! `cargo run --example signet_smoke` — talks to the network, not run in CI):
//! 1. `default(Testnet)` must reach the KaleidoSwap signet maker (de-Boltz).
//! 2. `default(Mainnet)` must error (no mainnet maker yet).
//! 3. Query the live pair/height surface.
//! 4. Create a reverse swap with SDK-derived keys + preimage and let the SDK
//!    cryptographically validate the returned lockup/tree (no funding).

use kaleidoswap_sdk::boltz::BoltzApiClientV2;
use kaleidoswap_sdk::network::Network;
use kaleidoswap_sdk::util::secrets::{Preimage, SwapMasterKey};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    // (2) mainnet default must be an error
    assert!(
        BoltzApiClientV2::default(Network::Mainnet).is_err(),
        "mainnet default() should error until the mainnet maker is live"
    );
    println!("[ok] default(Mainnet) errors as designed");

    // (1) testnet default -> maker.signet.kaleidoswap.com/v2
    let api = BoltzApiClientV2::default(Network::Testnet).map_err(|e| format!("{e:?}"))?;
    println!("[ok] default(Testnet) constructed (KaleidoSwap signet maker)");

    // (3) live read surface. NB: /v2/chain/heights hangs on the stale
    // deployed maker (v0.1.0) — skipped, tracked as a deploy issue.
    let rev_pairs = api
        .get_reverse_pairs()
        .await
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "[ok] GET /v2/swap/reverse tolerated: {} reverse pairs (stale deploy advertises none)",
        rev_pairs.btc.len()
    );

    // (4) create + validate a SUBMARINE swap (BTC -> LN), unfunded.
    // The live signet maker (v0.1.0, stale) advertises no reverse pairs yet,
    // so we exercise the submarine path: construct a validly-signed throwaway
    // BOLT11 signet invoice, create the swap, and let the SDK validate the
    // returned lockup script/tree against the invoice preimage-hash + our
    // refund key. Never funded -> expires server-side.
    use kaleidoswap_sdk::bitcoin::secp256k1::{Secp256k1, SecretKey};
    use kaleidoswap_sdk::lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};
    use std::time::{SystemTime, UNIX_EPOCH};

    let master = SwapMasterKey::new(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        None,
        Network::Testnet,
    ).map_err(|e| format!("{e:?}"))?;
    let kp = master.derive_swapkey(0).map_err(|e| format!("{e:?}"))?;
    let refund_pk = kaleidoswap_sdk::bitcoin::PublicKey::new(kp.public_key());

    let sub_pairs = api
        .get_submarine_pairs()
        .await
        .map_err(|e| format!("{e:?}"))?;
    let sp = serde_json::to_value(&sub_pairs).map_err(|e| e.to_string())?;
    let min = sp["BTC"]["BTC"]["limits"]["minimal"]
        .as_u64()
        .ok_or("no BTC/BTC submarine pair".to_string())?;
    let amount_msat = min.max(50_000) * 1000;
    println!(
        "    submarine BTC/BTC minimal = {min} sats; using {} sats",
        amount_msat / 1000
    );

    // throwaway node key + preimage we control
    let secp = Secp256k1::new();
    let node_sk = SecretKey::from_slice(&[0x42u8; 32]).map_err(|e| e.to_string())?;
    let preimage = Preimage::from_swap_key(&kp);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let invoice = InvoiceBuilder::new(Currency::Signet)
        .description("kaleidoswap-sdk signet smoke (unfunded)".into())
        .payment_hash(preimage.sha256)
        .payment_secret(PaymentSecret([7u8; 32]))
        .amount_milli_satoshis(amount_msat)
        .duration_since_epoch(now)
        .min_final_cltv_expiry_delta(80)
        .build_signed(|h| secp.sign_ecdsa_recoverable(h, &node_sk))
        .map_err(|e| format!("{e:?}"))?;
    let invoice_str = invoice.to_string();
    println!(
        "[ok] built signed signet invoice ({} sats)",
        amount_msat / 1000
    );

    let req = kaleidoswap_sdk::boltz::CreateSubmarineRequest {
        from: "BTC".to_string(),
        to: "BTC".to_string(),
        invoice: invoice_str.clone(),
        refund_public_key: refund_pk,
        pair_hash: None,
        referral_id: None,
        webhook: None,
    };
    let resp = api
        .post_swap_req(&req)
        .await
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "[ok] created submarine swap id = {} (acceptZeroConf={})",
        resp.id, resp.accept_zero_conf
    );

    // The critical step: SDK-side cryptographic validation of the maker's
    // lockup script/tree against the invoice preimage-hash + our refund key.
    resp.validate(
        &invoice_str,
        &refund_pk,
        kaleidoswap_sdk::network::Chain::Bitcoin(
            kaleidoswap_sdk::network::BitcoinChain::BitcoinTestnet,
        ),
    )
    .map_err(|e| format!("{e:?}"))?;
    println!(
        "[ok] response VALIDATED: lockup address + swap tree match invoice hash + our refund key"
    );
    println!(
        "    lockup: {} (expected {} sats)",
        resp.address, resp.expected_amount
    );
    println!("    bip21:  {}", resp.bip21);

    println!("\nSMOKE PASSED — SDK speaks to the live signet maker end-to-end");
    Ok(())
}
