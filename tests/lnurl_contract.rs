#![cfg(feature = "lnurl")]
use kaleidorg_swap_sdk::{lnurl::lnurl::LnUrl, util::lnurl::resolve_lightning_address};

#[tokio::test]
async fn public_resolver_rejects_insecure_clearnet_before_network_io() {
    // Each refusal names its rule: a non-public destination is refused as
    // such (whatever its scheme), plain clearnet HTTP for the scheme, and a
    // URL with credentials or a fragment for carrying them. Port 1 and the
    // metadata address make sure a request that did go out would fail loudly.
    for (url, expected) in [
        ("http://127.0.0.1:1/pay", "not a public destination"),
        ("https://127.0.0.1:1/pay", "not a public destination"),
        (
            "http://169.254.169.254/latest/meta-data",
            "not a public destination",
        ),
        ("http://example.com/pay", "HTTPS"),
        (
            "https://user:secret@example.com/pay",
            "credentials or fragments",
        ),
        (
            "https://example.com/pay#fragment",
            "credentials or fragments",
        ),
    ] {
        let lnurl = LnUrl::from_url(url.to_owned()).encode();
        let message = resolve_lightning_address(&lnurl, 1)
            .await
            .unwrap_err()
            .message();
        assert!(message.contains(expected), "{url}: {message}");
    }
}

#[tokio::test]
async fn invalid_satoshi_amounts_and_addresses_fail_before_network_io() {
    for amount in [0, u64::MAX, u64::MAX / 1000 + 1] {
        assert!(resolve_lightning_address("alice@example.invalid", amount)
            .await
            .is_err());
    }
    assert!(resolve_lightning_address("not-an-address", 1)
        .await
        .is_err());
}
