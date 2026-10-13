use crate::error::Error;
use bitcoin::hashes::Hash;
use lightning_invoice::Bolt11Invoice;
use lnurl::lightning_address::LightningAddress;
use lnurl::withdraw::WithdrawalResponse;
use lnurl::{lnurl::LnUrl, Builder, LnUrlResponse};
use std::str::FromStr;

/// Keep a genuine transport failure typed as [`Error::HTTP`], so the cause
/// under it survives, and do not claim the label for the rest.
///
/// `lnurl::Error` covers the whole client, not just its socket: `InvalidLnUrl`,
/// `HttpResponse(404)` and `Json` are answers, not failures to reach anyone,
/// and `Error::HTTP` now means specifically "no usable response came back".
/// Only `Reqwest` is that, and it carries the same `reqwest::Error` this
/// crate's own requests produce — `Cargo.lock` has a single `reqwest`, so
/// lnurl-rs and this crate resolve to one.
///
/// The others fold to text as before. Note that `lnurl::Error` renders through
/// `Debug` (its `Display` is `write!(f, "{:?}", self)`), so `HttpResponse(404)`
/// reads as `HttpResponse(404)`.
///
/// Enumerated rather than closed with a catch-all: a future lnurl-rs release
/// that adds a transport-shaped variant should fail to compile here instead of
/// landing silently in [`Error::Generic`]. `Ureq` is absent because this crate
/// takes lnurl-rs without its `blocking` feature.
///
/// `Io` and `Json` could each be carried by the matching variant of this enum
/// instead, which would keep their causes too; that is a separate change from
/// the one this function exists for, and is left deliberately.
fn from_lnurl_error(e: lnurl::Error) -> Error {
    match e {
        lnurl::Error::Reqwest(e) => Error::HTTP(e),
        e @ (lnurl::Error::InvalidLnUrl
        | lnurl::Error::InvalidLightningAddress
        | lnurl::Error::InvalidComment
        | lnurl::Error::InvalidAmount
        | lnurl::Error::HttpResponse(_)
        | lnurl::Error::Io(_)
        | lnurl::Error::Json(_)
        | lnurl::Error::InvalidResponse
        | lnurl::Error::Other(_)) => Error::Generic(e.to_string()),
    }
}

pub fn validate_lnurl(string: &str) -> bool {
    let string = string.to_lowercase();
    LnUrl::from_str(&string).is_ok() || LightningAddress::from_str(&string).is_ok()
}

/// Resolve a Lightning address or encoded LNURL-pay request to a fixed-amount
/// invoice. Amount is whole satoshis, unlike [`fetch_invoice`]'s millisatoshis.
pub async fn resolve_lightning_address(address: &str, amount_sat: u64) -> Result<String, Error> {
    let amount_msats = amount_sat
        .checked_mul(1_000)
        .filter(|amount| *amount > 0)
        .ok_or_else(|| {
            Error::Protocol("amountSat must be positive and fit in millisatoshis".into())
        })?;
    fetch_invoice(address, amount_msats).await
}

/// LNURL-pay transport policy: HTTPS on clearnet, HTTP only for onion services,
/// no credentials/fragments/redirects, and a deadline including the body read.
struct PayHttp {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    client: reqwest::Client,
    timeout: std::time::Duration,
    #[cfg(test)]
    allow_loopback: bool,
}

impl PayHttp {
    fn new() -> Result<Self, Error> {
        Ok(Self {
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            timeout: std::time::Duration::from_secs(30),
            #[cfg(test)]
            allow_loopback: false,
        })
    }

    fn url(&self, value: &str) -> Result<url::Url, Error> {
        let url = url::Url::parse(value)?;
        let host = url.host_str().unwrap_or_default();
        let secure =
            url.scheme() == "https" || (url.scheme() == "http" && host.ends_with(".onion"));
        #[cfg(test)]
        let secure =
            secure || (self.allow_loopback && url.scheme() == "http" && host == "127.0.0.1");
        if !secure
            || host.is_empty()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Protocol("LNURL requires HTTPS (HTTP only for onion services), without URL credentials or fragments".into()));
        }
        Ok(url)
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    async fn get(&self, url: &url::Url) -> Result<String, Error> {
        let response = self
            .client
            .get(url.clone())
            .timeout(self.timeout)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HTTPStatusNotSuccess(status, serde_json::Value::Null));
        }
        Ok(response.text().await?)
    }

    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    async fn get(&self, url: &url::Url) -> Result<String, Error> {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;
        fn failure(_: JsValue) -> Error {
            Error::Generic("LNURL fetch failed or exceeded its deadline".into())
        }
        struct AbortOnDrop(web_sys::AbortController);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let abort = AbortOnDrop(web_sys::AbortController::new().map_err(failure)?);
        let controller = abort.0.clone();
        let _timer = gloo_timers::callback::Timeout::new(
            self.timeout.as_millis().min(u32::MAX as u128) as u32,
            move || controller.abort(),
        );
        let init = web_sys::RequestInit::new();
        init.set_redirect(web_sys::RequestRedirect::Error);
        init.set_credentials(web_sys::RequestCredentials::Omit);
        init.set_signal(Some(&abort.0.signal()));
        let request =
            web_sys::Request::new_with_str_and_init(url.as_str(), &init).map_err(failure)?;
        let global = js_sys::global();
        let fetch = js_sys::Reflect::get(&global, &JsValue::from_str("fetch"))
            .map_err(failure)?
            .dyn_into::<js_sys::Function>()
            .map_err(failure)?;
        let promise = fetch
            .call1(&global, &request)
            .map_err(failure)?
            .dyn_into::<js_sys::Promise>()
            .map_err(failure)?;
        let response = JsFuture::from(promise)
            .await
            .map_err(failure)?
            .dyn_into::<web_sys::Response>()
            .map_err(failure)?;
        if response.redirected() || response.url() != url.as_str() {
            return Err(Error::Protocol("LNURL redirects are not supported".into()));
        }
        if !response.ok() {
            return Err(Error::HTTPStatusNotSuccess(
                reqwest::StatusCode::from_u16(response.status())
                    .map_err(|_| Error::Protocol("Invalid LNURL HTTP status".into()))?,
                serde_json::Value::Null,
            ));
        }
        JsFuture::from(response.text().map_err(failure)?)
            .await
            .map_err(failure)?
            .as_string()
            .ok_or_else(|| Error::Protocol("LNURL body is not text".into()))
    }
}

fn provider_error(body: &str) -> Result<(), Error> {
    let value: serde_json::Value = serde_json::from_str(body)?;
    if value.get("status").and_then(serde_json::Value::as_str) == Some("ERROR") {
        return Err(Error::Protocol(format!(
            "LNURL provider rejected request: {}",
            value
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unspecified reason")
        )));
    }
    Ok(())
}

pub async fn fetch_invoice(address: &str, amount_msats: u64) -> Result<String, Error> {
    fetch_invoice_with_http(address, amount_msats, &PayHttp::new()?).await
}

async fn fetch_invoice_with_http(
    address: &str,
    amount_msats: u64,
    http: &PayHttp,
) -> Result<String, Error> {
    let address = address.to_lowercase();
    let lnurl = LnUrl::from_str(&address)
        .or_else(|_| LightningAddress::from_str(&address).map(|value| value.lnurl()))
        .map_err(|_| Error::Protocol("Not a valid LNURL or Lightning address".into()))?;
    let url = http.url(&lnurl.url)?;
    let body = http.get(&url).await?;
    provider_error(&body)?;
    let pay = match lnurl::decode_ln_url_response(&body).map_err(from_lnurl_error)? {
        LnUrlResponse::LnUrlPayResponse(pay) => pay,
        _ => return Err(Error::Protocol("Expected an LNURL-pay request".into())),
    };
    if amount_msats == 0
        || pay.min_sendable == 0
        || pay.min_sendable > pay.max_sendable
        || amount_msats < pay.min_sendable
        || amount_msats > pay.max_sendable
    {
        return Err(Error::Protocol(format!(
            "LNURL amount must be between {} and {} millisatoshis",
            pay.min_sendable, pay.max_sendable
        )));
    }
    let mut callback = http.url(&pay.callback)?;
    callback
        .query_pairs_mut()
        .append_pair("amount", &amount_msats.to_string());
    let body = http.get(&callback).await?;
    provider_error(&body)?;
    let result: lnurl::pay::LnURLPayInvoice = serde_json::from_str(&body)?;
    let invoice = Bolt11Invoice::from_str(result.invoice()).map_err(Error::Bolt11)?;
    if invoice.amount_milli_satoshis() != Some(amount_msats) {
        return Err(Error::Protocol(
            "Invoice amount doesn't match requested amount".into(),
        ));
    }
    match invoice.description() {
        lightning_invoice::Bolt11InvoiceDescriptionRef::Hash(hash)
            if hash.0.to_byte_array() == pay.metadata_hash() => {}
        _ => {
            return Err(Error::Protocol(
                "Invoice description_hash does not match LNURL metadata".into(),
            ))
        }
    }
    Ok(result.invoice().to_owned())
}

pub async fn create_withdraw_response(voucher: &str) -> Result<WithdrawalResponse, Error> {
    let lnurl = LnUrl::from_str(&voucher.to_lowercase())
        .map_err(|_| Error::Generic("Invalid LNURL".to_string()))?;

    let client = Builder::default()
        .build_async()
        .map_err(|e| Error::Generic(e.to_string()))?;

    let res = client
        .make_request(&lnurl.url)
        .await
        .map_err(from_lnurl_error)?;

    match res {
        LnUrlResponse::LnUrlWithdrawResponse(withdraw) => Ok(withdraw),
        _ => Err(Error::Generic("Unexpected response type".to_string())),
    }
}

pub async fn process_withdrawal(withdraw: &WithdrawalResponse, invoice: &str) -> Result<(), Error> {
    let client = Builder::default()
        .build_async()
        .map_err(|e| Error::Generic(e.to_string()))?;

    client
        .do_withdrawal(withdraw, invoice)
        .await
        .map_err(from_lnurl_error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_url_policy() {
        let http = PayHttp::new().unwrap();
        for url in [
            "https://pay.example/a?token=kept",
            "http://service.onion/pay",
        ] {
            assert!(http.url(url).is_ok());
        }
        for url in [
            "http://pay.example/a",
            "http://127.0.0.1/pay",
            "https://user:secret@pay.example/a",
            "https://pay.example/a#fragment",
            "file:///etc/passwd",
        ] {
            assert!(http.url(url).is_err());
        }
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn secure_server(
        replies: Vec<(u16, String)>,
        delay_ms: u64,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let thread_base = base.clone();
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let mut buffer = [0; 1024];
                    let count = stream.read(&mut buffer).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                let body = body.replace("{BASE}", &thread_base);
                let _ = write!(stream, "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body);
            }
            requests
        });
        (LnUrl::from_url(format!("{base}/pay")).encode(), server)
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn secure_pay(callback: &str, metadata: &str) -> String {
        serde_json::json!({"tag":"payRequest", "callback":callback, "minSendable":1000, "maxSendable":200_000_000, "metadata":metadata}).to_string()
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[tokio::test]
    async fn secure_resolution_metadata_limits_and_provider_errors() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/lnurl-v1/invoice.json"))
                .unwrap();
        let invoice = fixture["invoice"].as_str().unwrap();
        let metadata = "[[\"text/plain\",\"test\"]]";
        let mut http = PayHttp::new().unwrap();
        http.allow_loopback = true;
        for (requested, description, expected) in [
            (100_000_000, metadata, None),
            (
                100_001_000,
                metadata,
                Some("doesn't match requested amount"),
            ),
            (
                100_000_000,
                "[[\"text/plain\",\"different payee\"]]",
                Some("description_hash"),
            ),
        ] {
            let (address, server) = secure_server(
                vec![
                    (200, secure_pay("{BASE}/invoice?token=kept", description)),
                    (200, serde_json::json!({"pr": invoice}).to_string()),
                ],
                0,
            );
            let result = fetch_invoice_with_http(&address, requested, &http).await;
            match expected {
                None => assert_eq!(result.unwrap(), invoice),
                Some(text) => assert!(result.unwrap_err().message().contains(text)),
            }
            let requests = server.join().unwrap();
            assert!(
                requests[1].starts_with(&format!("GET /invoice?token=kept&amount={requested} "))
            );
        }
        for replies in [
            vec![(
                200,
                r#"{"status":"ERROR","reason":"Service unavailable"}"#.into(),
            )],
            vec![
                (200, secure_pay("{BASE}/invoice", metadata)),
                (
                    200,
                    r#"{"status":"ERROR","reason":"Service unavailable"}"#.into(),
                ),
            ],
        ] {
            let (address, server) = secure_server(replies, 0);
            assert!(fetch_invoice_with_http(&address, 100_000_000, &http)
                .await
                .unwrap_err()
                .message()
                .contains("Service unavailable"));
            server.join().unwrap();
        }
        let (address, server) = secure_server(
            vec![(200, secure_pay("http://pay.example/invoice", metadata))],
            0,
        );
        assert!(fetch_invoice_with_http(&address, 100_000_000, &http)
            .await
            .unwrap_err()
            .message()
            .contains("HTTPS"));
        assert_eq!(server.join().unwrap().len(), 1);
        let (address, server) =
            secure_server(vec![(200, secure_pay("{BASE}/invoice", metadata))], 0);
        assert!(fetch_invoice_with_http(&address, 1, &http)
            .await
            .unwrap_err()
            .message()
            .contains("between 1000 and 200000000"));
        assert_eq!(server.join().unwrap().len(), 1);
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[tokio::test]
    async fn secure_transport_rejects_redirects_and_bounds_stalls() {
        let mut http = PayHttp::new().unwrap();
        http.allow_loopback = true;
        let (address, server) = secure_server(vec![(302, String::new())], 0);
        assert!(
            matches!(fetch_invoice_with_http(&address, 1000, &http).await.unwrap_err(), Error::HTTPStatusNotSuccess(status, _) if status.as_u16() == 302)
        );
        server.join().unwrap();
        http.timeout = std::time::Duration::from_millis(30);
        let (address, server) = secure_server(vec![(200, "{}".into())], 150);
        let start = std::time::Instant::now();
        assert!(
            matches!(fetch_invoice_with_http(&address, 1000, &http).await.unwrap_err(), Error::HTTP(error) if error.is_timeout())
        );
        assert!(start.elapsed() < std::time::Duration::from_millis(140));
        server.join().unwrap();
    }

    /// The reclassification, and the cause it exists to keep.
    ///
    /// `lnurl::Error`'s `Display` is `{:?}`, and reqwest's `Debug` recurses
    /// into `source`, so the old `Error::HTTP(e.to_string())` happened to
    /// carry the whole chain inside one string. Holding the error is only an
    /// improvement if the chain is still reachable — through `source()` for a
    /// Rust caller, and through `message_with_causes` for a string-only one.
    /// `message` alone renders strictly less than the old string did, which is
    /// why the consumer above moved.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[macros::async_test]
    async fn a_transport_failure_stays_http_and_keeps_its_cause() {
        // Same shape as the refused-connection test in `crate::error`:
        // loopback, no proxy so an answering one cannot turn this into a
        // response, and bounded so a dropped connection cannot hang.
        let refused = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("a client with no proxy builds")
            .get("http://127.0.0.1:1/")
            .send()
            .await
            .expect_err("nothing listens on loopback port 1");
        let old_flattened = lnurl::Error::Reqwest(refused).to_string();

        let refused = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("a client with no proxy builds")
            .get("http://127.0.0.1:1/")
            .send()
            .await
            .expect_err("nothing listens on loopback port 1");
        let error = from_lnurl_error(lnurl::Error::Reqwest(refused));

        assert_eq!(error.name(), "HTTP", "a transport failure lost its variant");
        assert!(
            std::error::Error::source(&error).is_some(),
            "no cause below reqwest's own layer"
        );
        // The old string carried the refusal; the fold still has to.
        assert!(
            old_flattened.to_lowercase().contains("refused"),
            "the premise no longer holds: {old_flattened}"
        );
        assert!(
            error
                .message_with_causes()
                .to_lowercase()
                .contains("refused"),
            "the refusal is gone from the folded text: {}",
            error.message_with_causes()
        );
    }

    /// An answer, not a failure to reach anyone, so not `Error::HTTP`.
    #[test]
    fn an_answer_is_not_a_transport_failure() {
        for (case, expected_text) in [
            (lnurl::Error::HttpResponse(404), "HttpResponse(404)"),
            (lnurl::Error::InvalidLnUrl, "InvalidLnUrl"),
            (lnurl::Error::InvalidResponse, "InvalidResponse"),
        ] {
            let error = from_lnurl_error(case);
            assert_eq!(error.name(), "Generic", "wrong variant for {expected_text}");
            assert_eq!(
                error.message(),
                expected_text,
                "the text lnurl always rendered must survive"
            );
            assert!(std::error::Error::source(&error).is_none());
        }
    }

    async fn test_address(address: &str, amount_msats: u64, format: &str) {
        let result = fetch_invoice(address, amount_msats).await;

        match result {
            Ok(invoice) => {
                assert!(!invoice.is_empty(), "Invoice should not be empty");
                assert!(
                    invoice.starts_with("lnbc"),
                    "Invoice should start with 'lnbc'"
                );
                println!("Successfully fetched invoice, format : {format}")
            }
            Err(e) => {
                println!(
                    "Error occured with {} format: {}",
                    format,
                    e.message_with_causes()
                );
            }
        }
    }

    #[macros::async_test_all]
    async fn test_fetch_invoice() {
        let amount_msats = 100000;
        let lnurl = "lnurl1dp68gurn8ghj7um9wfmxjcm99e3k7mf0v9cxj0m385ekvcenxc6r2c35xvukxefcv5mkvv34x5ekzd3ev56nyd3hxqurzepexejxxepnxscrvwfnv9nxzcn9xq6xyefhvgcxxcmyxymnserxfq5fns";
        let uppercase_lnurl = lnurl.to_uppercase();
        assert!(validate_lnurl(lnurl));
        test_address(lnurl, amount_msats, "LNURL").await;
        test_address(&uppercase_lnurl, amount_msats, "LNURL").await;

        let email_lnurl = "drunksteel17@walletofsatoshi.com";
        assert!(validate_lnurl(email_lnurl));
        test_address(email_lnurl, amount_msats, "Lightning Address").await;
    }

    #[ignore = "Requires using an new lnurl-w voucher and invoice to match the max_withdrawble amount"]
    #[macros::async_test_all]
    async fn test_process_withdrawal() {
        let voucher = "LNURL1DP68GURN8GHJ7ER9D4HJUMRWVF5HGUEWVDHK6TMHD96XSERJV9MJ7CTSDYHHVVF0D3H82UNV9AVYS6ZV899XS4J6WFYRV6Z9TQU4GUT9VF48SWY20AR";
        let invoice = "lnbc4u1pnsywcypp5eamm4c3v42vlyr0asmt55muv02zusjp2dy7j6e3kuz5vv3cuyj6scqpjsp56hujjsj4r76gp9gk6y435rz99682uxjx924a06wwqm0av6ezxepq9q7sqqqqqqqqqqqqqqqqqqqsqqqqqysgqdqqmqz9gxqyjw5qrzjqwryaup9lh50kkranzgcdnn2fgvx390wgj5jd07rwr3vxeje0glcllm8u4a8gvusysqqqqlgqqqqqeqqjqtgxt57vzea9xaygxu806xf7w5872n737ptuc6al0plf3544a2f5y2e42j9qv7gvkqkn9k2yxzmew6rr40z2gyq9nu8atj2yt4dlfm3gpjevcgu";
        assert!(validate_lnurl(voucher));
        let withdraw_response = match create_withdraw_response(voucher).await {
            Ok(response) => response,
            Err(e) => {
                println!("Failed to create withdraw response: {e:?}");
                return;
            }
        };

        let invoice_amount = match Bolt11Invoice::from_str(invoice) {
            Ok(invoice) => invoice.amount_milli_satoshis(),
            Err(e) => {
                println!("Failed to parse invoice: {e:?}");
                return;
            }
        };

        assert_eq!(
            invoice_amount,
            Option::from(withdraw_response.max_withdrawable),
            "Invoice of {:?} doesn't match with withdrawable {} sats",
            invoice_amount,
            withdraw_response.max_withdrawable
        );
        println!("Successfully created withdraw response{withdraw_response:?}");
        let result = process_withdrawal(&withdraw_response, invoice).await;

        assert!(result.is_ok(), "Withdrawal failed: {:?}", result.err());

        println!("Withdrawal test passed successfully");
    }
}
