use bitcoin::base64;
use bitcoin::base64::Engine;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use futures::FutureExt;
#[cfg(feature = "ws")]
use kaleidorg_swap_sdk::boltz::BoltzWsApi;
use kaleidorg_swap_sdk::network::{BitcoinChain, Chain, LiquidChain};
use kaleidorg_swap_sdk::util::sleep;
use reqwest::Client;
use serde_json::{json, Value};
use std::{error::Error, sync::Arc, time::Duration};

const BITCOIND_URL: &str = "http://localhost:18443/wallet/client";
const ELEMENTSD_URL: &str = "http://localhost:18884/wallet/client";
const LND_URL: &str = "https://localhost:8081";

const PROXY_URL: &str = "http://localhost:51234/proxy";

const BITCOIND_COOKIE: Option<&str> = option_env!("BITCOIND_COOKIE");
const ELEMENTSD_COOKIE: &str = "regtest:regtest";
const LND_MACAROON_HEX: Option<&str> = option_env!("LND_MACAROON_HEX");

async fn json_rpc_request(
    chain: Chain,
    method: &str,
    params: Value,
) -> Result<Value, Box<dyn Error>> {
    let (url, cookie) = match chain {
        Chain::Bitcoin(_) => (BITCOIND_URL, BITCOIND_COOKIE.unwrap()),
        Chain::Liquid(_) => (ELEMENTSD_URL, ELEMENTSD_COOKIE),
    };

    let client = Client::new();

    let req_body = json!({
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": method,
        "params": params
    });

    let res = client
        .post(PROXY_URL)
        .header(
            "Authorization",
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(cookie)
            ),
        )
        .header("X-Proxy-URL", url)
        .json(&req_body)
        .send()
        .await?
        .json::<Value>()
        .await?;

    res.get("result")
        .cloned()
        .ok_or_else(|| "Invalid response".into())
}

/// Posts to LND's REST gateway and hands back the raw body.
///
/// Kept separate from [`lnd_request`] because not every LND endpoint answers
/// with a single JSON document — see [`pay_invoice_lnd_inner`].
async fn lnd_post(method: &str, params: Value) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let url = format!("{LND_URL}/{method}");

    let body = client
        .post(PROXY_URL)
        .header("Grpc-Metadata-macaroon", LND_MACAROON_HEX.unwrap())
        .header("X-Proxy-URL", url)
        .json(&params)
        .send()
        .await?
        .text()
        .await?;

    Ok(body)
}

/// Calls a *unary* LND endpoint, whose body is one JSON document.
async fn lnd_request(method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
    let body = lnd_post(method, params).await?;
    Ok(serde_json::from_str(&body)?)
}

pub async fn generate_address(chain: Chain) -> Result<String, Box<dyn Error>> {
    json_rpc_request(chain, "getnewaddress", json!([]))
        .await?
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid response".into())
}

pub async fn get_blinding_key(chain: Chain, address: &str) -> Result<String, Box<dyn Error>> {
    let result = json_rpc_request(chain, "dumpblindingkey", json!([address])).await?;
    Ok(result.as_str().unwrap().to_string())
}

pub async fn send_to_address(
    chain: Chain,
    address: &str,
    sat_amount: u64,
) -> Result<String, Box<dyn Error>> {
    let btc_amount = (sat_amount as f64) / 100_000_000.0;
    let params = json!([address, format!("{:.8}", btc_amount)]);
    json_rpc_request(chain, "sendtoaddress", params)
        .await?
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid response".into())
}

pub async fn generate_invoice_lnd(amount_sat: u64) -> Result<String, Box<dyn Error>> {
    let response = lnd_request("v1/invoices", json!({ "value": amount_sat })).await?;
    response["payment_request"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing payment_request field".into())
}

/// Starts a payment through LND's `SendPaymentV2`.
///
/// `v2/router/send` is a *server-streaming* RPC, so grpc-gateway answers with
/// one JSON object per line rather than a single document. Decoding the whole
/// body as JSON therefore fails with `trailing characters, line 2, column 1`
/// as soon as LND emits a second payment update (typically `IN_FLIGHT` then
/// `SUCCEEDED`) — which is a race, not a payment failure. Read the first
/// object, which carries any immediate rejection, and ignore the rest of the
/// stream: callers only need the payment to have been accepted.
pub async fn pay_invoice_lnd_inner(invoice: &str) -> Result<(), Box<dyn Error>> {
    let body = lnd_post(
        "v2/router/send",
        json!({ "payment_request": invoice, "timeout_seconds": 1 }),
    )
    .await?;

    check_send_payment_response(&body)
}

/// Reads the first update out of a `SendPaymentV2` body.
///
/// An empty body means LND produced no update at all, which the caller treats
/// the same as an accepted payment: whichever swap status the payment should
/// produce is what the test actually asserts on.
fn check_send_payment_response(body: &str) -> Result<(), Box<dyn Error>> {
    let Some(first) = body.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return Ok(());
    };

    let update: Value = serde_json::from_str(first)?;
    if let Some(error) = update.get("error").filter(|e| !e.is_null()) {
        return Err(format!("LND rejected the payment: {error}").into());
    }

    Ok(())
}

#[cfg(feature = "ws")]
pub fn start_ws(ws: Arc<BoltzWsApi>) {
    let future = ws.run_ws_loop();

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        tokio::spawn(future);
    }

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        // In WASM, we can use spawn_local since we don't need Send
        wasm_bindgen_futures::spawn_local(future);
    }
}

pub fn start_pay_invoice_lnd(invoice: String) {
    let task = async move {
        // Deliberately not `unwrap()`. This runs detached, so nothing awaits
        // it and a panic here has nowhere to surface as a test failure. On
        // wasm it is worse than useless: the panic leaves the executor's
        // `Task` mid-poll, its `RefCell<Option<Inner>>` still mutably
        // borrowed and the guard never dropped, so the next wake of that task
        // panics with `already borrowed: BorrowMutError` inside js-sys —
        // attributed to whichever test happens to be running by then.
        //
        // A payment that never lands is not lost signal either: every caller
        // goes on to await the swap status it should produce, and fails on
        // that timeout with a message that names what it was waiting for.
        if let Err(e) = pay_invoice_lnd_inner(&invoice).await {
            log::error!("Failed to pay invoice through LND: {e}");
        }
    };

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        tokio::spawn(task);
    }

    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        wasm_bindgen_futures::spawn_local(async {
            let timeout_future = gloo_timers::future::TimeoutFuture::new(5000);
            let _ = futures::select! {
                _ = task.fuse() => {},
                _ = timeout_future.fuse() => {},
            };
        });
    }
}

pub async fn mine_blocks(n_blocks: u64) -> Result<(), Box<dyn Error>> {
    for chain in [
        BitcoinChain::BitcoinRegtest.into(),
        LiquidChain::LiquidRegtest.into(),
    ] {
        let address = generate_address(chain).await?;
        json_rpc_request(chain, "generatetoaddress", json!([n_blocks, address])).await?;
    }
    Ok(())
}

/// How often the regtest Boltz backend batch-sweeps the claims it deferred.
///
/// `regtest/boltz/data/backend/boltz.conf` does not set
/// `swap.batchClaimInterval`, so the backend's default cron applies:
/// `*/15 * * * *`, on the minute, every quarter hour. Every time zone offset is
/// a whole number of quarter hours, so the boundaries fall on multiples of 15
/// minutes in Unix time whatever the container's zone.
const BOLTZ_BATCH_CLAIM_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Margin either side of a sweep for the host and container clocks to disagree.
const BOLTZ_SWEEP_CLOCK_SKEW: Duration = Duration::from_secs(5);

/// Waits, if need be, until no Boltz batch sweep can fire in the next `window`.
///
/// Once Boltz pays a submarine swap's invoice it defers its own claim and
/// reports `transaction.claim.pending`. The swap then waits in the backend's
/// `DeferredClaimer` until we send a partial signature for a cooperative claim
/// or the next scheduled sweep claims it on its own. If the sweep wins, Boltz
/// answers our signature with `400 swap not eligible for a cooperative claim
/// broadcast`. That is correct server behaviour, and the swap still completes,
/// but the test fails. Run 34591230856 hit this at 11:00:00 UTC, on a sweep.
///
/// Tolerating that error would let the cooperative path go untested whenever
/// the race is lost. Starting the swap clear of a sweep keeps it under test on
/// every run.
///
/// Only the interval sweep is guarded against, because on the pinned
/// `regtest/boltz` it is the only sweep trigger that can fire. Neither
/// `sweepAmountTrigger` nor `scheduleAmountTrigger` is set. `ExpiryTrigger`
/// needs 120 minutes or less to expiry, and taproot submarine swaps get 10080.
/// If a submodule bump changes any of that, the same 400 can come back from
/// another trigger. It fails loudly, like this one did, and needs its own
/// guard.
pub async fn wait_out_boltz_batch_sweep(window: Duration) {
    if let Some(wait) = batch_sweep_wait(unix_now(), window) {
        log::info!(
            "Waiting {}s so a Boltz batch sweep cannot claim the swap before we do",
            wait.as_secs()
        );
        sleep(wait).await;
    }
}

/// How long to wait at `now` (since the Unix epoch) so the next `window` holds
/// no sweep, allowing for clock skew. `None` when it is clear already.
fn batch_sweep_wait(now: Duration, window: Duration) -> Option<Duration> {
    let interval = BOLTZ_BATCH_CLAIM_INTERVAL.as_millis();
    let skew = BOLTZ_SWEEP_CLOCK_SKEW.as_millis();
    let since_sweep = now.as_millis() % interval;
    let until_sweep = interval - since_sweep;

    let wait_ms = if since_sweep < skew {
        // A sweep has just fired, or is about to on the container's clock.
        skew - since_sweep
    } else if until_sweep < window.as_millis() + skew {
        until_sweep + skew
    } else {
        return None;
    };

    Some(Duration::from_millis(wait_ms as u64))
}

fn unix_now() -> Duration {
    // `SystemTime::now()` panics on wasm32-unknown-unknown; ask the browser.
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        Duration::from_millis(js_sys::Date::now() as u64)
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
    }
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod batch_sweep_wait_tests {
    use super::{batch_sweep_wait, BOLTZ_BATCH_CLAIM_INTERVAL, BOLTZ_SWEEP_CLOCK_SKEW};
    use std::time::Duration;

    const WINDOW: Duration = Duration::from_secs(60);

    /// Some quarter-hour boundary: 2026-09-11T11:00:00Z, when #59's run failed.
    const SWEEP: Duration = Duration::from_secs(1_789_124_400);

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn the_boundary_constant_is_a_sweep() {
        assert_eq!(SWEEP.as_secs() % BOLTZ_BATCH_CLAIM_INTERVAL.as_secs(), 0);
    }

    #[test]
    fn does_not_wait_mid_interval() {
        assert_eq!(batch_sweep_wait(SWEEP + secs(7 * 60), WINDOW), None);
    }

    /// The run in #59 started its swap about 1.5s before the 11:00 sweep.
    #[test]
    fn waits_past_an_imminent_sweep() {
        let now = SWEEP - Duration::from_millis(1_500);
        let wait = batch_sweep_wait(now, WINDOW).unwrap();
        assert_eq!(now + wait, SWEEP + BOLTZ_SWEEP_CLOCK_SKEW);
    }

    #[test]
    fn waits_for_a_sweep_that_just_fired_to_clear_the_skew() {
        let now = SWEEP + secs(2);
        let wait = batch_sweep_wait(now, WINDOW).unwrap();
        assert_eq!(now + wait, SWEEP + BOLTZ_SWEEP_CLOCK_SKEW);
    }

    #[test]
    fn the_skew_widens_the_window() {
        let edge = SWEEP - WINDOW - BOLTZ_SWEEP_CLOCK_SKEW;
        assert_eq!(batch_sweep_wait(edge, WINDOW), None);
        assert!(batch_sweep_wait(edge + Duration::from_millis(1), WINDOW).is_some());
    }

    /// Whatever the starting point, the swap then has the whole window clear.
    #[test]
    fn leaves_the_window_clear_everywhere() {
        let interval = BOLTZ_BATCH_CLAIM_INTERVAL.as_secs();
        let skew = BOLTZ_SWEEP_CLOCK_SKEW.as_secs();
        for offset in 0..interval {
            let now = SWEEP + secs(offset);
            let start = now + batch_sweep_wait(now, WINDOW).unwrap_or_default();
            let since = start.as_secs() % interval;
            assert!(
                since >= skew && interval - since >= WINDOW.as_secs() + skew,
                "starting at +{offset}s leaves the swap {since}s past a sweep",
            );
        }
    }
}

// Pure body parsing, so the native run covers it; nothing here is
// wasm-specific and the wasm suite stays exactly as it was.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod send_payment_response_tests {
    use super::check_send_payment_response;

    const IN_FLIGHT: &str = r#"{"result":{"payment_hash":"ab","status":"IN_FLIGHT"}}"#;
    const SUCCEEDED: &str = r#"{"result":{"payment_hash":"ab","status":"SUCCEEDED"}}"#;

    #[test]
    fn accepts_a_single_update() {
        check_send_payment_response(IN_FLIGHT).unwrap();
    }

    /// The regression: `SendPaymentV2` streams, so two updates arrive as two
    /// JSON documents on separate lines. Decoding the body as one document
    /// used to fail with `trailing characters, line 2, column 1`.
    #[test]
    fn accepts_a_multi_line_stream() {
        let body = format!("{IN_FLIGHT}\n{SUCCEEDED}\n");
        check_send_payment_response(&body).unwrap();
    }

    #[test]
    fn accepts_an_empty_body() {
        check_send_payment_response("").unwrap();
        check_send_payment_response("\n\n").unwrap();
    }

    #[test]
    fn reports_a_rejection() {
        let body = r#"{"error":{"code":2,"message":"invoice is already paid"}}"#;
        let err = check_send_payment_response(body).unwrap_err().to_string();
        assert!(err.contains("invoice is already paid"), "got: {err}");
    }

    #[test]
    fn reports_an_undecodable_first_line() {
        check_send_payment_response("not json").unwrap_err();
    }
}
