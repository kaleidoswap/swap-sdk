//! Manual probe for issue #319: does a swap created through the SDK's
//! `KaleidoMakerClient` actually arrive attributed at the maker?
//!
//! Run against the regtest stack:
//!   MAKER_URL=http://127.0.0.1:9420/v2 KALEIDOSWAP_API_KEY=kld_test_… \
//!     cargo run --example kaleido_attribution_probe

use std::str::FromStr;

use kaleidorg_swap_sdk::error::Error;
use kaleidorg_swap_sdk::swaps::boltz::{BoltzApiClientV2, CreateReverseRequest};
use kaleidorg_swap_sdk::swaps::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};

use kaleidorg_swap_sdk::bitcoin::hashes::{sha256, Hash};
use kaleidorg_swap_sdk::PublicKey;

fn reverse_request(preimage_seed: &str, amount: u64) -> CreateReverseRequest {
    CreateReverseRequest {
        // BTC@LN -> L-BTC: one of the reverse routes this maker actually
        // publishes for SDK callers (`GET /v2/swap/reverse`). The internal
        // `pairId: "BTC/BTC"` form curl can use is not an SDK route.
        from: "BTC".to_string(),
        to: "L-BTC".to_string(),
        claim_public_key: PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("static claim key parses"),
        invoice: None,
        invoice_amount: Some(amount),
        preimage_hash: Some(sha256::Hash::hash(preimage_seed.as_bytes())),
        description: None,
        description_hash: None,
        address: None,
        address_signature: None,
        referral_id: None,
        webhook: None,
        pair_hash: None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let maker_url =
        std::env::var("MAKER_URL").unwrap_or_else(|_| "http://127.0.0.1:9420/v2".to_string());
    let raw_key = std::env::var("KALEIDOSWAP_API_KEY").expect("KALEIDOSWAP_API_KEY is required");

    // 1. The key parses, and neither Debug nor the redacted form leaks the secret.
    let api_key = ApiKey::parse(&raw_key)?;
    println!("1. key parsed");
    println!("   environment : {}", api_key.environment());
    println!("   key_id      : {}", api_key.key_id());
    println!("   redacted    : {}", api_key.redacted());
    println!("   Debug       : {api_key:?}");
    let secret = raw_key.rsplit('_').next().unwrap_or_default();
    let debug = format!("{api_key:?}");
    assert!(
        !debug.contains(secret) && !api_key.redacted().contains(secret),
        "the secret escaped into Debug or redacted()"
    );
    println!("   secret absent from both: yes");

    // 2. Origin binding: a non-loopback plaintext maker URL is refused outright.
    let plaintext_remote = KaleidoMakerClient::new(KaleidoMakerClientOptions {
        maker_url: "http://maker.example.com/v2".to_string(),
        api_key: api_key.clone(),
        timeout: None,
    });
    match plaintext_remote {
        Err(error) => println!("\n2. plaintext non-loopback maker refused: {error}"),
        Ok(_) => panic!("a plaintext remote maker URL was accepted"),
    }

    // 3. Build the real client and talk to the maker.
    let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
        maker_url: maker_url.clone(),
        api_key: api_key.clone(),
        timeout: None,
    })?;
    let height = client.get_height().await?;
    println!("\n3. client built for {maker_url}");
    println!("   get_height  : btc={} lbtc={}", height.btc, height.lbtc);

    // 4. Create a reverse swap with the key attached.
    let attributed = client
        .post_reverse_req(reverse_request("sdk-probe-attributed", 100_000))
        .await?;
    println!("\n4. attributed create via KaleidoMakerClient");
    println!("   swap id     : {}", attributed.id);
    println!("   lockup      : {}", attributed.lockup_address);
    println!("   onchain amt : {}", attributed.onchain_amount);

    // 5. The same call through the generic Boltz client carries no key, so the
    //    maker must record it as anonymous.
    let anonymous = BoltzApiClientV2::new(maker_url, None)
        .post_reverse_req(reverse_request("sdk-probe-anonymous", 100_000))
        .await?;
    println!("\n5. anonymous create via BoltzApiClientV2");
    println!("   swap id     : {}", anonymous.id);

    // 6. A tampered key must be refused by the maker, not silently downgraded.
    let mut tampered_raw = raw_key.clone();
    tampered_raw.pop();
    tampered_raw.push('x');
    let tampered = KaleidoMakerClient::new(KaleidoMakerClientOptions {
        maker_url: std::env::var("MAKER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9420/v2".to_string()),
        api_key: ApiKey::parse(&tampered_raw)?,
        timeout: None,
    })?;
    match tampered
        .post_reverse_req(reverse_request("sdk-probe-tampered", 100_000))
        .await
    {
        Ok(response) => panic!("a tampered key created swap {}", response.id),
        Err(Error::HTTP(message)) => println!("\n6. tampered key refused: HTTP {message}"),
        Err(error) => println!("\n6. tampered key refused: {error}"),
    }

    println!("\nswap ids for the DB check:");
    println!("  attributed = {}", attributed.id);
    println!("  anonymous  = {}", anonymous.id);
    Ok(())
}
