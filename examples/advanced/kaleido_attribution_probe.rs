//! Manual probe for issue #319: does a swap created through the SDK's
//! `KaleidoMakerClient` actually arrive attributed at the maker, and does a key
//! the maker does not recognise degrade to an anonymous swap rather than fail?
//!
//! Run against the regtest stack:
//!   MAKER_URL=http://127.0.0.1:9420/v2 KALEIDOSWAP_API_KEY=kld_test_… \
//!     cargo run --example kaleido_attribution_probe

use std::str::FromStr;

use kaleidorg_swap_sdk::error::Error;
use kaleidorg_swap_sdk::swaps::boltz::{BoltzApiClientV2, CreateReverseRequest};
use kaleidorg_swap_sdk::swaps::kaleido::{
    ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions, MakerAttribution, API_KEY_PREFIX,
};

use kaleidorg_swap_sdk::bitcoin::hashes::{sha256, Hash};
use kaleidorg_swap_sdk::bitcoin::secp256k1::rand;
use kaleidorg_swap_sdk::PublicKey;

/// A reverse-swap request whose payment hash is unique to this run.
///
/// `run` is mixed into the preimage seed because the maker turns `preimage_hash`
/// into a hold invoice, and a Lightning backend refuses to issue a second
/// invoice for a payment hash it has already used. With a fixed seed the probe
/// works once against a persistent regtest stack and every later run dies at
/// step 4 — before a single attribution check has run — with an error about the
/// invoice rather than about attribution.
fn reverse_request(preimage_seed: &str, run: u64, amount: u64) -> CreateReverseRequest {
    let preimage_seed = format!("{preimage_seed}-{run:016x}");
    CreateReverseRequest {
        // BTC@LN -> L-BTC: one of the reverse routes this maker actually
        // publishes for SDK callers (`GET /v2/swap/reverse`). The internal
        // `pairId: "BTC@LN/BTC@L1"` form curl can use is not an SDK route.
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
    // One nonce for the whole run, so the three payment hashes below are fresh
    // and this run's swaps are greppable in the maker's logs.
    let run: u64 = rand::random();
    println!("run id      : {run:016x}");
    let raw_key = std::env::var("KALEIDOSWAP_API_KEY").expect("KALEIDOSWAP_API_KEY is required");
    // The same trim `ApiKey::parse` applies. Every check below compares against
    // the bytes the type actually holds, and a key read from a file or a shell
    // heredoc routinely arrives with a trailing newline — which, left on, makes
    // the secret needle a string that cannot appear anywhere and turns the leak
    // assertion into a no-op that always passes.
    let raw_key = raw_key
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .to_string();

    // 1. The key parses, and neither Debug nor the redacted form leaks the secret.
    let api_key = ApiKey::parse(&raw_key)?;
    println!("1. key parsed");
    println!("   environment : {}", api_key.environment());
    println!("   key_id      : {}", api_key.key_id());
    println!("   redacted    : {}", api_key.redacted());
    println!("   Debug       : {api_key:?}");
    // The whole secret, not the tail after the last `_`: the secret may be
    // base64url, where `_` is an ordinary byte, so a needle taken from the last
    // segment is a suffix — and for a secret ending in `_` it is the empty
    // string, which every haystack contains. Rebuilding the prefix from the
    // public accessors is the same split `ApiKey::parse` performed.
    let prefix = format!(
        "{API_KEY_PREFIX}_{}_{}_",
        api_key.environment(),
        api_key.key_id()
    );
    let secret = raw_key
        .strip_prefix(&prefix)
        .expect("a parsed key is its own redacted prefix followed by its secret");
    assert!(
        !secret.is_empty(),
        "the secret needle came out empty, so the check below would prove nothing"
    );
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
        .post_reverse_req(reverse_request("sdk-probe-attributed", run, 100_000))
        .await?;
    println!("\n4. attributed create via KaleidoMakerClient");
    println!("   swap id     : {}", attributed.id);
    println!("   lockup      : {}", attributed.lockup_address);
    println!("   onchain amt : {}", attributed.onchain_amount);
    assert_eq!(
        client.last_attribution(),
        Some(MakerAttribution::ApiKey),
        "the maker did not answer the keyed create with X-Kaleido-Attribution: api_key"
    );
    println!("   attribution : api_key");

    // 5. The same call through the generic Boltz client carries no key, so the
    //    maker must record it as anonymous.
    let anonymous = BoltzApiClientV2::new(maker_url.clone(), None)
        .post_reverse_req(reverse_request("sdk-probe-anonymous", run, 100_000))
        .await?;
    println!("\n5. anonymous create via BoltzApiClientV2");
    println!("   swap id     : {}", anonymous.id);

    // 6. A well-formed key the maker does not recognise is not an error: the
    //    key is a public identifier that ships inside apps and can outlive a
    //    partnership, so the maker prices and creates anonymously and says so in
    //    `X-Kaleido-Attribution: none`.
    //
    // The last byte is flipped to something it is not, rather than assigned a
    // fixed character: `pop()` followed by `push('x')` leaves a key ending in
    // `x` byte-for-byte identical, and the probe would then send the genuine
    // key and report an attributed swap as an unrecognised one.
    let mut tampered_raw = raw_key.clone();
    let last = tampered_raw.pop().expect("a parsed key is not empty");
    tampered_raw.push(if last == 'x' { 'y' } else { 'x' });
    assert_ne!(
        tampered_raw, raw_key,
        "the tampered key is the real key, so this step would test nothing"
    );
    let tampered = KaleidoMakerClient::new(KaleidoMakerClientOptions {
        maker_url: maker_url.clone(),
        api_key: ApiKey::parse(&tampered_raw)?,
        timeout: None,
    })?;
    let unrecognised = tampered
        .post_reverse_req(reverse_request("sdk-probe-tampered", run, 100_000))
        .await
        .map_err(|error| {
            format!(
                "an unrecognised key must create anonymously, not fail ({}): {error}",
                error.name()
            )
        })?;
    assert_eq!(
        tampered.last_attribution(),
        Some(MakerAttribution::Unattributed),
        "an unrecognised key's create was not answered X-Kaleido-Attribution: none"
    );
    println!("\n6. unrecognised key created anonymously");
    println!("   swap id     : {}", unrecognised.id);
    println!("   attribution : none");

    // 7. A malformed `Authorization` is not a key at all, and stays a `401
    //    partner_auth_invalid`. `ApiKey::parse` refuses to build one, so this
    //    goes out through a plain HTTP client.
    let malformed = kaleidorg_swap_sdk::reqwest::Client::new()
        .post(format!("{maker_url}/swap/reverse"))
        .header("Authorization", "Bearer not-a-partner-key")
        .json(&reverse_request("sdk-probe-malformed", run, 100_000))
        .send()
        .await?;
    let status = malformed.status();
    let body = malformed.text().await?;
    assert_eq!(
        status,
        kaleidorg_swap_sdk::reqwest::StatusCode::UNAUTHORIZED,
        "a malformed Authorization was answered {status}, not 401: {body}"
    );
    assert!(
        body.contains("partner_auth_invalid"),
        "a malformed Authorization's 401 does not name partner_auth_invalid: {body}"
    );
    println!("\n7. malformed Authorization refused: {status}");

    // 8. A keyed create carrying the anonymous card's `pairHash` is refused:
    //    the keyed card, which includes the partner fee, has a hash of its own.
    let anonymous_hash = BoltzApiClientV2::new(maker_url.clone(), None)
        .get_reverse_pairs()
        .await?
        .get("BTC", "L-BTC")
        .map(|card| card.hash.clone())
        .ok_or("the maker publishes no BTC -> L-BTC reverse card")?;
    let keyed_hash = client
        .get_reverse_pairs()
        .await?
        .get("BTC", "L-BTC")
        .map(|card| card.hash.clone())
        .ok_or("the maker publishes no keyed BTC -> L-BTC reverse card")?;
    if anonymous_hash == keyed_hash {
        println!("\n8. skipped: the key carries no partner fee, so both cards share one hash");
    } else {
        let mut mismatched = reverse_request("sdk-probe-hash", run, 100_000);
        mismatched.pair_hash = Some(anonymous_hash);
        match client.post_reverse_req(mismatched).await {
            Ok(response) => panic!(
                "a keyed create with the anonymous card's hash created swap {}",
                response.id
            ),
            Err(Error::HTTPStatusNotSuccess(status, body))
                if status == kaleidorg_swap_sdk::reqwest::StatusCode::BAD_REQUEST
                    && body["error"] == "pair_hash_mismatch" =>
            {
                println!("\n8. anonymous hash on a keyed create refused: {status}")
            }
            Err(error) => panic!(
                "expected 400 pair_hash_mismatch for the anonymous hash on a keyed \
                 create, got {}: {error}",
                error.name()
            ),
        }
    }

    println!("\nswap ids for the DB check:");
    println!("  attributed = {}", attributed.id);
    println!("  anonymous  = {}", anonymous.id);
    println!("  unrecognised key = {}", unrecognised.id);
    Ok(())
}
