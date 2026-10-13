#![cfg(feature = "lnurl")]
use kaleidorg_swap_sdk::{lnurl::lnurl::LnUrl, util::lnurl::resolve_lightning_address};

#[tokio::test]
async fn public_resolver_rejects_insecure_clearnet_before_network_io() {
    for url in [
        "http://127.0.0.1:1/pay",
        "http://169.254.169.254/latest/meta-data",
        "http://example.com/pay",
        "https://user:secret@example.com/pay",
        "https://example.com/pay#fragment",
    ] {
        let lnurl = LnUrl::from_url(url.to_owned()).encode();
        assert!(resolve_lightning_address(&lnurl, 1)
            .await
            .unwrap_err()
            .message()
            .contains("HTTPS"));
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
