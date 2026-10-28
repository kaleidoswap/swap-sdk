use crate::error::Error;
use bitcoin::hashes::Hash;
use lightning_invoice::Bolt11Invoice;
use lnurl::lightning_address::LightningAddress;
use lnurl::withdraw::WithdrawalResponse;
use lnurl::{lnurl::LnUrl, LnUrlResponse};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
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
    resolve_lightning_address_with_options(address, amount_sat, &LnurlOptions::default()).await
}

/// [`resolve_lightning_address`] with an explicit egress policy.
pub async fn resolve_lightning_address_with_options(
    address: &str,
    amount_sat: u64,
    options: &LnurlOptions,
) -> Result<String, Error> {
    let amount_msats = amount_sat
        .checked_mul(1_000)
        .filter(|amount| *amount > 0)
        .ok_or_else(|| {
            Error::Protocol("amountSat must be positive and fit in millisatoshis".into())
        })?;
    fetch_invoice_with_options(address, amount_msats, options).await
}

/// Egress policy for every LNURL request this module makes (pay and withdraw,
/// discovery and callback alike).
///
/// The default refuses destinations inside the caller's own network: an LNURL
/// or Lightning address is attacker-chosen input, and a server resolving one
/// would otherwise issue requests to services reachable only from where it
/// runs (loopback, RFC 1918, link-local, CGNAT and the other reserved ranges,
/// for IPv4 and IPv6 including IPv4-mapped addresses). The check runs on the
/// literal host before any request and, on native, on every address a
/// hostname resolves to, with the connection pinned to exactly those vetted
/// addresses so a rebinding name cannot answer differently at connect time.
/// In a browser the user agent makes the connection, so only the literal-host
/// rules apply there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LnurlOptions {
    /// Allow loopback, private, link-local and other non-public destinations,
    /// and plain HTTP to them. For development against a local LNURL service
    /// only; never set it on a server that resolves addresses it did not pick.
    pub allow_private_hosts: bool,
}

/// Why an address is not a public destination, or `None` when it is one.
///
/// Explicit ranges rather than the `Ipv4Addr`/`Ipv6Addr` classification
/// helpers, several of which are unstable or newer than the pinned toolchain,
/// and so the list is readable in one place.
pub fn blocked_ip_reason(ip: IpAddr) -> Option<&'static str> {
    fn v4(ip: Ipv4Addr) -> Option<&'static str> {
        let [a, b, _, _] = ip.octets();
        Some(match (a, b) {
            (0, _) => "unspecified/this-network",
            (10, _) => "private",
            (100, 64..=127) => "shared/CGNAT",
            (127, _) => "loopback",
            (169, 254) => "link-local",
            (172, 16..=31) => "private",
            (192, 0) if ip.octets()[2] == 0 => "IETF protocol assignments",
            (192, 0) if ip.octets()[2] == 2 => "documentation",
            (192, 88) if ip.octets()[2] == 99 => "6to4 relay anycast",
            (192, 168) => "private",
            (198, 18..=19) => "benchmarking",
            (198, 51) if ip.octets()[2] == 100 => "documentation",
            (203, 0) if ip.octets()[2] == 113 => "documentation",
            (224..=239, _) => "multicast",
            (240..=255, _) => "reserved/broadcast",
            _ => return None,
        })
    }
    fn v6(ip: Ipv6Addr) -> Option<&'static str> {
        if let Some(mapped) = ip.to_ipv4_mapped() {
            return v4(mapped);
        }
        let segments = ip.segments();
        if ip.is_unspecified() {
            return Some("unspecified");
        }
        if ip.is_loopback() {
            return Some("loopback");
        }
        // Deprecated IPv4-compatible (::a.b.c.d) and 6to4 (2002::/16) embed an
        // IPv4 address; judge that address.
        if segments[..6] == [0, 0, 0, 0, 0, 0] {
            return Some("IPv4-compatible");
        }
        if segments[0] == 0x2002 {
            let [a, b] = segments[1].to_be_bytes();
            let [c, d] = segments[2].to_be_bytes();
            return v4(Ipv4Addr::new(a, b, c, d));
        }
        Some(match segments[0] {
            0x0100 if segments[1..4] == [0, 0, 0] => "discard-only",
            0x2001 if segments[1] == 0 => "Teredo",
            0x2001 if segments[1] == 0x2 && segments[2] == 0 => "benchmarking",
            0x2001 if (0x10..=0x2f).contains(&segments[1]) => "ORCHID",
            0x2001 if segments[1] == 0xdb8 => "documentation",
            0x3ffe => "6bone",
            s if s & 0xfe00 == 0xfc00 => "unique local",
            s if s & 0xffc0 == 0xfe80 => "link-local",
            s if s & 0xffc0 == 0xfec0 => "site-local",
            s if s & 0xff00 == 0xff00 => "multicast",
            _ => return None,
        })
    }
    match ip {
        IpAddr::V4(ip) => v4(ip),
        IpAddr::V6(ip) => v6(ip),
    }
}

/// Resolves names with the system resolver and refuses the whole lookup when
/// any answer is a non-public address, so reqwest connects only to vetted
/// addresses. Bound to the client, it runs for every new connection: a name
/// that rebinds between discovery and callback is judged again.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
struct EgressResolver {
    allow_private_hosts: bool,
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl reqwest::dns::Resolve for EgressResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let allow_private_hosts = self.allow_private_hosts;
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if addrs.is_empty() {
                return Err(format!("LNURL host {host} resolved to no addresses").into());
            }
            if !allow_private_hosts {
                if let Some((addr, reason)) = addrs
                    .iter()
                    .find_map(|addr| blocked_ip_reason(addr.ip()).map(|reason| (addr, reason)))
                {
                    return Err(format!(
                        "LNURL host {host} resolves to a {reason} address ({}), which is not a public destination",
                        addr.ip()
                    )
                    .into());
                }
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// LNURL transport policy, shared by pay and withdraw: HTTPS on clearnet, HTTP
/// only for onion services, no credentials/fragments/redirects, public
/// destinations only (see [`LnurlOptions`]), and a deadline including the
/// body read.
struct LnurlHttp {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    client: reqwest::Client,
    timeout: std::time::Duration,
    allow_private_hosts: bool,
}

impl LnurlHttp {
    fn new(options: &LnurlOptions) -> Result<Self, Error> {
        Ok(Self {
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .dns_resolver(std::sync::Arc::new(EgressResolver {
                    allow_private_hosts: options.allow_private_hosts,
                }))
                .build()?,
            timeout: std::time::Duration::from_secs(30),
            allow_private_hosts: options.allow_private_hosts,
        })
    }

    fn url(&self, value: &str) -> Result<url::Url, Error> {
        let url = url::Url::parse(value)?;
        let host = url.host_str().unwrap_or_default();
        if host.is_empty()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Protocol(
                "LNURL requires a host and no URL credentials or fragments".into(),
            ));
        }
        // A literal address is judged here; a name is judged by the resolver
        // on the native client, and by nothing on wasm, where the browser
        // connects. `localhost` is a name, but one whose answer is known.
        let literal = match url.host() {
            Some(url::Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
            Some(url::Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
            Some(url::Host::Domain(name))
                if name == "localhost" || name.ends_with(".localhost") =>
            {
                Some(IpAddr::V4(Ipv4Addr::LOCALHOST))
            }
            _ => None,
        };
        let private = literal.and_then(blocked_ip_reason);
        if let Some(reason) = private {
            if !self.allow_private_hosts {
                return Err(Error::Protocol(format!(
                    "LNURL destination {host} is a {reason} address, which is not a public destination"
                )));
            }
        }
        let secure = url.scheme() == "https"
            || (url.scheme() == "http"
                && (host.ends_with(".onion") || (self.allow_private_hosts && private.is_some())));
        if !secure {
            return Err(Error::Protocol(
                "LNURL requires HTTPS (HTTP only for onion services)".into(),
            ));
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
    fetch_invoice_with_options(address, amount_msats, &LnurlOptions::default()).await
}

/// [`fetch_invoice`] with an explicit egress policy.
pub async fn fetch_invoice_with_options(
    address: &str,
    amount_msats: u64,
    options: &LnurlOptions,
) -> Result<String, Error> {
    fetch_invoice_with_http(address, amount_msats, &LnurlHttp::new(options)?).await
}

async fn fetch_invoice_with_http(
    address: &str,
    amount_msats: u64,
    http: &LnurlHttp,
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

/// Fetch an LNURL-withdraw request (LUD-03) over the hardened transport.
///
/// The voucher's URL and the returned `callback` both have to pass the
/// [`LnurlOptions`] policy, `k1` has to be present and the withdrawable
/// bounds sane; the result is otherwise refused before any invoice is sent.
pub async fn create_withdraw_response(voucher: &str) -> Result<WithdrawalResponse, Error> {
    create_withdraw_response_with_options(voucher, &LnurlOptions::default()).await
}

/// [`create_withdraw_response`] with an explicit egress policy.
pub async fn create_withdraw_response_with_options(
    voucher: &str,
    options: &LnurlOptions,
) -> Result<WithdrawalResponse, Error> {
    create_withdraw_response_with_http(voucher, &LnurlHttp::new(options)?).await
}

async fn create_withdraw_response_with_http(
    voucher: &str,
    http: &LnurlHttp,
) -> Result<WithdrawalResponse, Error> {
    let lnurl = LnUrl::from_str(&voucher.to_lowercase())
        .map_err(|_| Error::Protocol("Not a valid LNURL".into()))?;
    let url = http.url(&lnurl.url)?;
    let body = http.get(&url).await?;
    provider_error(&body)?;
    let withdraw = match lnurl::decode_ln_url_response(&body).map_err(from_lnurl_error)? {
        LnUrlResponse::LnUrlWithdrawResponse(withdraw) => withdraw,
        _ => return Err(Error::Protocol("Expected an LNURL-withdraw request".into())),
    };
    validate_withdraw(&withdraw, http)?;
    Ok(withdraw)
}

/// The withdraw context the callback is bound to: a callback URL that passes
/// the egress policy, a `k1` to present, and limits an invoice can meet.
fn validate_withdraw(withdraw: &WithdrawalResponse, http: &LnurlHttp) -> Result<url::Url, Error> {
    let callback = http.url(&withdraw.callback)?;
    if withdraw.k1.is_empty() {
        return Err(Error::Protocol("LNURL-withdraw request has no k1".into()));
    }
    let min = withdraw.min_withdrawable.unwrap_or(1);
    if min == 0 || min > withdraw.max_withdrawable {
        return Err(Error::Protocol(format!(
            "LNURL-withdraw limits are invalid: min {min} > max {} millisatoshis",
            withdraw.max_withdrawable
        )));
    }
    Ok(callback)
}

/// Present `invoice` to the withdraw service's callback (LUD-03) over the
/// hardened transport.
///
/// `k1` is a replayable withdrawal capability, so the callback goes out only
/// to a destination that passes the [`LnurlOptions`] policy, with redirects
/// refused; the invoice must carry an amount within the request's bounds.
pub async fn process_withdrawal(withdraw: &WithdrawalResponse, invoice: &str) -> Result<(), Error> {
    process_withdrawal_with_options(withdraw, invoice, &LnurlOptions::default()).await
}

/// [`process_withdrawal`] with an explicit egress policy.
pub async fn process_withdrawal_with_options(
    withdraw: &WithdrawalResponse,
    invoice: &str,
    options: &LnurlOptions,
) -> Result<(), Error> {
    process_withdrawal_with_http(withdraw, invoice, &LnurlHttp::new(options)?).await
}

async fn process_withdrawal_with_http(
    withdraw: &WithdrawalResponse,
    invoice: &str,
    http: &LnurlHttp,
) -> Result<(), Error> {
    let mut callback = validate_withdraw(withdraw, http)?;
    let parsed = Bolt11Invoice::from_str(invoice).map_err(Error::Bolt11)?;
    let amount_msats = parsed
        .amount_milli_satoshis()
        .ok_or_else(|| Error::Protocol("LNURL-withdraw invoice must carry an amount".into()))?;
    let min = withdraw.min_withdrawable.unwrap_or(1);
    if amount_msats < min || amount_msats > withdraw.max_withdrawable {
        return Err(Error::Protocol(format!(
            "LNURL-withdraw amount must be between {min} and {} millisatoshis",
            withdraw.max_withdrawable
        )));
    }
    callback
        .query_pairs_mut()
        .append_pair("k1", &withdraw.k1)
        .append_pair("pr", invoice);
    let body = http.get(&callback).await?;
    provider_error(&body)?;
    let value: serde_json::Value = serde_json::from_str(&body)?;
    let acknowledged = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("OK"));
    if !acknowledged {
        return Err(Error::Protocol(
            "LNURL-withdraw provider did not acknowledge the withdrawal".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_http(allow_private_hosts: bool) -> LnurlHttp {
        LnurlHttp::new(&LnurlOptions {
            allow_private_hosts,
        })
        .unwrap()
    }

    #[test]
    fn secure_url_policy() {
        let http = test_http(false);
        for url in [
            "https://pay.example/a?token=kept",
            "http://service.onion/pay",
            "https://[2606:4700::1111]/pay",
            "https://1.1.1.1/pay",
        ] {
            assert!(http.url(url).is_ok(), "{url}");
        }
        for url in [
            "http://pay.example/a",
            "http://127.0.0.1/pay",
            "https://user:secret@pay.example/a",
            "https://pay.example/a#fragment",
            "file:///etc/passwd",
        ] {
            assert!(http.url(url).is_err(), "{url}");
        }
    }

    /// Finding 9: a literal non-public destination is refused before any
    /// request, over HTTPS too, unless the caller opted in.
    #[test]
    fn literal_private_destinations_are_refused_by_default() {
        let http = test_http(false);
        for (url, reason) in [
            ("https://127.0.0.1/pay", "loopback"),
            ("https://127.8.8.8:8080/pay", "loopback"),
            ("https://localhost/pay", "loopback"),
            ("https://api.localhost/pay", "loopback"),
            ("https://10.1.2.3/pay", "private"),
            ("https://172.16.0.1/pay", "private"),
            ("https://172.31.255.255/pay", "private"),
            ("https://192.168.1.1/pay", "private"),
            ("https://169.254.169.254/latest/meta-data", "link-local"),
            ("https://100.64.0.1/pay", "shared/CGNAT"),
            ("https://0.0.0.0/pay", "unspecified"),
            ("https://224.0.0.1/pay", "multicast"),
            ("https://255.255.255.255/pay", "reserved"),
            ("https://[::1]/pay", "loopback"),
            ("https://[::]/pay", "unspecified"),
            ("https://[::ffff:127.0.0.1]/pay", "loopback"),
            ("https://[::ffff:10.0.0.1]/pay", "private"),
            ("https://[::ffff:169.254.169.254]/pay", "link-local"),
            ("https://[fc00::1]/pay", "unique local"),
            ("https://[fd12:3456::1]/pay", "unique local"),
            ("https://[fe80::1]/pay", "link-local"),
            ("https://[ff02::1]/pay", "multicast"),
            ("https://[2002:7f00:1::1]/pay", "loopback"),
            ("https://[2001:db8::1]/pay", "documentation"),
        ] {
            let error = http.url(url).unwrap_err().message();
            assert!(
                error.contains("not a public destination") && error.contains(reason),
                "{url}: {error}"
            );
        }
        // Public literals stay allowed; the same private ones pass on opt-in.
        for ip in ["8.8.8.8", "172.32.0.1", "100.128.0.1", "[2606:4700::1111]"] {
            assert!(http.url(&format!("https://{ip}/pay")).is_ok(), "{ip}");
            assert_eq!(
                blocked_ip_reason(ip.trim_matches(['[', ']']).parse().unwrap()),
                None
            );
        }
        let permissive = test_http(true);
        for url in [
            "https://127.0.0.1/pay",
            "http://127.0.0.1:3000/pay",
            "http://localhost:3000/pay",
            "https://[::1]/pay",
            "http://10.0.0.5/pay",
        ] {
            assert!(permissive.url(url).is_ok(), "{url}");
        }
        // Opting in to private hosts does not open plain HTTP to the clearnet.
        assert!(permissive.url("http://pay.example/a").is_err());
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
        let http = test_http(true);
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
        let mut http = test_http(true);
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

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn withdraw_request(callback: &str, k1: &str, min: Option<u64>, max: u64) -> String {
        let mut value = serde_json::json!({"tag":"withdrawRequest", "callback":callback, "k1":k1, "defaultDescription":"test", "maxWithdrawable":max});
        if let Some(min) = min {
            value["minWithdrawable"] = min.into();
        }
        value.to_string()
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn fixture_invoice() -> String {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/lnurl-v1/invoice.json"))
                .unwrap();
        fixture["invoice"].as_str().unwrap().to_owned()
    }

    /// Finding 6: withdraw discovery and the k1-bearing callback go through
    /// the same transport as pay, and the callback is bound to the validated
    /// request.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[tokio::test]
    async fn withdraw_runs_over_the_hardened_transport() {
        let invoice = fixture_invoice();
        let http = test_http(true);
        let (voucher, server) = secure_server(
            vec![
                (
                    200,
                    withdraw_request(
                        "{BASE}/cb?session=kept",
                        "k1secret",
                        Some(1000),
                        100_000_000,
                    ),
                ),
                (200, r#"{"status":"OK"}"#.into()),
            ],
            0,
        );
        let withdraw = create_withdraw_response_with_http(&voucher, &http)
            .await
            .unwrap();
        assert_eq!(withdraw.k1, "k1secret");
        process_withdrawal_with_http(&withdraw, &invoice, &http)
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /pay "));
        assert!(
            requests[1].starts_with(&format!("GET /cb?session=kept&k1=k1secret&pr={invoice} ")),
            "{}",
            requests[1]
        );

        // The same flow through a name, so the resolver is on the path.
        let (voucher, server) = secure_server(
            vec![
                (200, withdraw_request("{BASE}/cb", "k1", None, 100_000_000)),
                (200, r#"{"status":"ok"}"#.into()),
            ],
            0,
        );
        let port = url::Url::parse(&LnUrl::from_str(&voucher).unwrap().url)
            .unwrap()
            .port()
            .unwrap();
        let voucher = LnUrl::from_url(format!("http://localhost:{port}/pay")).encode();
        let mut withdraw = create_withdraw_response_with_http(&voucher, &http)
            .await
            .unwrap();
        withdraw.callback = format!("http://localhost:{port}/cb");
        process_withdrawal_with_http(&withdraw, &invoice, &http)
            .await
            .unwrap();
        assert_eq!(server.join().unwrap().len(), 2);
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[tokio::test]
    async fn withdraw_refuses_unsafe_callbacks_and_bad_terms() {
        let invoice = fixture_invoice();
        let http = test_http(true);
        // Discovery answers that are refused before the callback is used.
        for (reply, expected) in [
            (
                withdraw_request("http://pay.example/cb", "k1", None, 100_000_000),
                "HTTPS",
            ),
            (
                withdraw_request("{BASE}/cb", "", None, 100_000_000),
                "no k1",
            ),
            (
                withdraw_request("{BASE}/cb", "k1", Some(2_000), 1_000),
                "limits are invalid",
            ),
            (
                withdraw_request("{BASE}/cb", "k1", Some(0), 1_000),
                "limits are invalid",
            ),
            (
                secure_pay("{BASE}/invoice", "[]"),
                "Expected an LNURL-withdraw",
            ),
            (
                r#"{"status":"ERROR","reason":"voucher spent"}"#.into(),
                "voucher spent",
            ),
        ] {
            let (voucher, server) = secure_server(vec![(200, reply)], 0);
            let error = create_withdraw_response_with_http(&voucher, &http)
                .await
                .unwrap_err()
                .message();
            assert!(error.contains(expected), "{expected}: {error}");
            assert_eq!(server.join().unwrap().len(), 1);
        }
        // A plain-clearnet callback is refused even by a permissive client,
        // and a private one by the default client.
        let strict = test_http(false);
        let (voucher, server) = secure_server(
            vec![(
                200,
                withdraw_request("https://10.0.0.1/cb", "k1", None, 100_000_000),
            )],
            0,
        );
        let withdraw = {
            let lnurl = LnUrl::from_str(&voucher).unwrap();
            let http = &http;
            let body = http.get(&http.url(&lnurl.url).unwrap()).await.unwrap();
            server.join().unwrap();
            match lnurl::decode_ln_url_response(&body).unwrap() {
                LnUrlResponse::LnUrlWithdrawResponse(withdraw) => withdraw,
                _ => unreachable!(),
            }
        };
        assert!(process_withdrawal_with_http(&withdraw, &invoice, &strict)
            .await
            .unwrap_err()
            .message()
            .contains("not a public destination"));

        // Callback-time refusals: the invoice has to fit the bounds, and the
        // service has to acknowledge.
        let (voucher, server) = secure_server(
            vec![(
                200,
                withdraw_request("{BASE}/cb", "k1", Some(1000), 100_000_000),
            )],
            0,
        );
        let withdraw = create_withdraw_response_with_http(&voucher, &http)
            .await
            .unwrap();
        server.join().unwrap();
        let mut small = withdraw.clone();
        small.max_withdrawable = 99_999_000;
        assert!(process_withdrawal_with_http(&small, &invoice, &http)
            .await
            .unwrap_err()
            .message()
            .contains("between 1000 and 99999000"));
        assert!(
            process_withdrawal_with_http(&withdraw, "not an invoice", &http)
                .await
                .is_err()
        );
        for (status, body, expected) in [
            (
                200,
                r#"{"status":"ERROR","reason":"already used"}"#,
                "already used",
            ),
            (200, r#"{"foo":"bar"}"#, "did not acknowledge"),
            (302, "", "302"),
        ] {
            let (voucher, server) = secure_server(vec![(status, body.into())], 0);
            let port = url::Url::parse(&LnUrl::from_str(&voucher).unwrap().url)
                .unwrap()
                .port()
                .unwrap();
            let mut withdraw = withdraw.clone();
            withdraw.callback = format!("http://127.0.0.1:{port}/cb");
            let error = process_withdrawal_with_http(&withdraw, &invoice, &http)
                .await
                .unwrap_err();
            assert!(
                error.message().contains(expected),
                "{expected}: {}",
                error.message()
            );
            server.join().unwrap();
        }
    }

    /// Finding 9: a name that resolves to a non-public address is refused by
    /// the resolver the native client connects through.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[tokio::test]
    async fn resolver_refuses_names_that_answer_with_private_addresses() {
        use reqwest::dns::Resolve;
        let name = || reqwest::dns::Name::from_str("localhost").unwrap();
        let strict = EgressResolver {
            allow_private_hosts: false,
        };
        let error = match strict.resolve(name()).await {
            Ok(_) => panic!("localhost resolved"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("loopback") && error.contains("not a public destination"),
            "{error}"
        );
        let addrs: Vec<_> = EgressResolver {
            allow_private_hosts: true,
        }
        .resolve(name())
        .await
        .unwrap()
        .collect();
        assert!(!addrs.is_empty());
        assert!(addrs.iter().all(|addr| addr.ip().is_loopback()));

        // End to end: the refusal surfaces as a transport error whose cause
        // names the policy, and no request is made.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let voucher = LnUrl::from_url(format!("https://localhost.:{port}/pay")).encode();
        let error = fetch_invoice_with_http(&voucher, 1000, &test_http(false))
            .await
            .unwrap_err();
        assert!(
            error
                .message_with_causes()
                .contains("not a public destination"),
            "{}",
            error.message_with_causes()
        );
        assert!(listener.accept().is_err(), "a connection was made");
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
