use crate::error::Error;
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

pub async fn fetch_invoice(address: &str, amount_msats: u64) -> Result<String, Error> {
    let address = address.to_lowercase();
    let lnurl = match LnUrl::from_str(&address) {
        Ok(lnurl) => lnurl,
        Err(_) => match LightningAddress::from_str(&address) {
            Ok(lightning_address) => lightning_address.lnurl(),
            Err(_) => return Err(Error::Generic("Not a valid LnUrl or LnAddress".to_string())),
        },
    };

    let client = Builder::default()
        .build_async()
        .map_err(|e| Error::Generic(e.to_string()))?;
    let res = client
        .make_request(&lnurl.url)
        .await
        .map_err(from_lnurl_error)?;

    match res {
        LnUrlResponse::LnUrlPayResponse(pay) => {
            let pay_result = client
                .get_invoice(&pay, amount_msats, None, None)
                .await
                .map_err(from_lnurl_error)?;
            let invoice = Bolt11Invoice::from_str(pay_result.invoice()).map_err(Error::Bolt11)?;

            if invoice.amount_milli_satoshis() != Some(amount_msats) {
                return Err(Error::Generic(
                    "Invoice amount doesn't match requested amount".to_string(),
                ));
            }

            Ok(pay_result.invoice().to_string())
        }
        _ => Err(Error::Generic("Unexpected response type".to_string())),
    }
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
