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

// Keep proxy environment variables isolated from concurrent tests.
#[test]
fn environment_proxy_cannot_bypass_destination_validation() {
    const CHILD: &str = "SDK_LNURL_PROXY_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let lnurl = LnUrl::from_url("https://localhost.:443/pay".to_owned()).encode();
        assert!(runtime
            .block_on(resolve_lightning_address(&lnurl, 1))
            .is_err());
        return;
    }
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    proxy.set_nonblocking(true).unwrap();
    let address = format!("http://{}", proxy.local_addr().unwrap());
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "environment_proxy_cannot_bypass_destination_validation",
        ])
        .env(CHILD, "1")
        .env("HTTPS_PROXY", &address)
        .env("https_proxy", &address)
        .env("ALL_PROXY", &address)
        .env("all_proxy", &address)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match proxy.accept() {
            Ok(_) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("LNURL request reached the environment proxy, bypassing destination DNS validation");
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("{e}"),
        }
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("isolated LNURL lookup did not finish");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
