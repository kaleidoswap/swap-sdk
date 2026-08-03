//! Live smoke against the KaleidoSwap **signet** maker (run manually:
//! `cargo run --example signet_smoke` — talks to the network, not run in CI):
//! 1. `default(Signet)` must reach the KaleidoSwap maker (de-Boltz).
//! 2. `default(Mainnet)` must error (no mainnet maker yet).
//! 3. Query the live pair surface.
//! 4. The maker's chain tip and the SDK's *default* signet chain access must
//!    agree — the invariant that `Network::Signet` exists to protect. Under the
//!    old testnet3 default this step is what would have caught the mismatch.
//!    (Needs the `esplora` feature, which is on by default.)
//! 5. Create a submarine swap with SDK-derived keys + preimage and let the SDK
//!    cryptographically validate the returned lockup/tree (no funding).

use kaleidorg_swap_sdk::boltz::BoltzApiClientV2;
use kaleidorg_swap_sdk::network::Network;
use kaleidorg_swap_sdk::util::secrets::{Preimage, SwapMasterKey};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    // (2) mainnet default must be an error
    assert!(
        BoltzApiClientV2::default(Network::Mainnet).is_err(),
        "mainnet default() should error until the mainnet maker is live"
    );
    println!("[ok] default(Mainnet) errors as designed");

    // (1) signet default -> maker.signet.kaleidoswap.com/v2
    let api = BoltzApiClientV2::default(Network::Signet).map_err(|e| format!("{e:?}"))?;
    println!("[ok] default(Signet) constructed (KaleidoSwap maker)");

    // (3) live read surface.
    let rev_pairs = api
        .get_reverse_pairs()
        .await
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "[ok] GET /v2/swap/reverse: {} reverse pairs",
        rev_pairs.btc.len()
    );

    // (4) maker chain tip vs the SDK's *default* signet chain access. These
    // must track the same chain. This is the check that would have caught the
    // testnet3 default: Mutinynet and testnet3 sat ~1.78M blocks apart when
    // this was written, yet their addresses are encoded identically, so nothing
    // downstream would have errored.
    #[cfg(feature = "esplora")]
    {
        use kaleidorg_swap_sdk::network::esplora::DEFAULT_SIGNET_NODE;

        let maker_height = api.get_height().await.map_err(|e| format!("{e:?}"))?.btc as i64;
        // The BitcoinClient trait has no height method, so read the tip from
        // the same constant EsploraBitcoinClient::default(BitcoinSignet) uses —
        // this asserts the constant itself points at the maker's chain.
        let chain_height: i64 = reqwest::get(format!("{DEFAULT_SIGNET_NODE}/blocks/tip/height"))
            .await
            .map_err(|e| format!("{e:?}"))?
            .text()
            .await
            .map_err(|e| format!("{e:?}"))?
            .trim()
            .parse()
            .map_err(|e| format!("unparseable tip height: {e:?}"))?;
        let drift = (maker_height - chain_height).abs();
        println!(
            "    maker BTC tip = {maker_height}, default signet chain tip = {chain_height} (drift {drift})"
        );
        // Mutinynet mines every ~30s, so a healthy pair sits within a handful
        // of blocks. A chain mismatch shows up as a drift of many thousands.
        if drift > 100 {
            return Err(format!(
                "maker and default signet chain access disagree by {drift} blocks — \
                 these are not the same chain"
            ));
        }
        println!("[ok] maker and default signet chain access agree on the tip");
    }

    // (5) create + validate a SUBMARINE swap (BTC -> LN), unfunded.
    // Submarine is the cheapest path to exercise end-to-end without funding:
    // construct a validly-signed throwaway
    // BOLT11 signet invoice, create the swap, and let the SDK validate the
    // returned lockup script/tree against the invoice preimage-hash + our
    // refund key. Never funded -> expires server-side.
    use kaleidorg_swap_sdk::bitcoin::secp256k1::{Secp256k1, SecretKey};
    use kaleidorg_swap_sdk::lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};
    use std::time::{SystemTime, UNIX_EPOCH};

    let master = SwapMasterKey::new(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        None,
        Network::Signet,
    ).map_err(|e| format!("{e:?}"))?;
    let kp = master.derive_swapkey(0).map_err(|e| format!("{e:?}"))?;
    let refund_pk = kaleidorg_swap_sdk::bitcoin::PublicKey::new(kp.public_key());

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
        .description("kaleidorg-swap-sdk signet smoke (unfunded)".into())
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

    let req = kaleidorg_swap_sdk::boltz::CreateSubmarineRequest {
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
        kaleidorg_swap_sdk::network::Chain::Bitcoin(
            kaleidorg_swap_sdk::network::BitcoinChain::BitcoinSignet,
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
