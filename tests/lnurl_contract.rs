#![cfg(feature = "lnurl")]

use kaleidorg_swap_sdk::lnurl::lnurl::LnUrl;
use kaleidorg_swap_sdk::util::lnurl::resolve_lightning_address;
use std::io::{Read, Write};

fn service(invoice: String) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let callback = format!("{base}/invoice?token=kept");
    let encoded = LnUrl::from_url(format!("{base}/pay")).encode();
    let server = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut buffer = [0; 1024];
                let count = stream.read(&mut buffer).unwrap();
                assert_ne!(count, 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            let body = if index == 0 {
                serde_json::json!({
                    "tag": "payRequest", "callback": callback,
                    "minSendable": 1000, "maxSendable": 200_000_000,
                    "metadata": "[[\"text/plain\",\"test\"]]"
                })
            } else {
                serde_json::json!({"pr": invoice})
            }
            .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
        requests
    });
    (encoded, server)
}

#[tokio::test]
async fn lnurl_resolution_preserves_fixed_amount_and_checks_returned_invoice() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lnurl-v1/invoice.json")).unwrap();
    let invoice = fixture["invoice"].as_str().unwrap();
    let (address, server) = service(invoice.to_owned());
    assert_eq!(
        resolve_lightning_address(&address, 100_000).await.unwrap(),
        invoice
    );
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /pay "));
    assert!(requests[1].starts_with("GET /invoice?token=kept&amount=100000000 "));

    let (address, server) = service(invoice.to_owned());
    let error = resolve_lightning_address(&address, 100_001)
        .await
        .unwrap_err();
    assert!(error.message().contains("doesn't match requested amount"));
    server.join().unwrap();
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
