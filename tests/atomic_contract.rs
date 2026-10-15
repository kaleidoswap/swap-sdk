//! The atomic client against a real maker's wire bodies
//! (`fixtures/atomic-v1`), replayed byte for byte.
#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

use std::io::{Read, Write};
use std::thread::JoinHandle;

use kaleidorg_swap_sdk::atomic::{
    AtomicAmountDirection, AtomicOutpoint, AtomicQuote, AtomicQuoteExpectations,
    AtomicQuoteRequest, AtomicSwapState, AtomicSwapStatus,
};
use kaleidorg_swap_sdk::boltz::BoltzApiClientV2;
use kaleidorg_swap_sdk::error::Error;
use serde_json::Value;

const CAPTURE: &str = include_str!("fixtures/atomic-v1/wire-capture.json");
const PAIRS: [&str; 2] = ["BTC/USDT-RGB", "USDT-RGB/BTC"];

fn capture(pair: &str) -> Value {
    let all: Value = serde_json::from_str(CAPTURE).unwrap();
    all["swaps"][pair].clone()
}

fn body(capture: &Value, name: &str) -> String {
    capture[name].as_str().expect("fixture body").to_owned()
}

fn json(capture: &Value, name: &str) -> Value {
    serde_json::from_str(&body(capture, name)).unwrap()
}

/// A maker that answers each request with the next raw body, and hands back
/// `METHOD PATH BODY` for every request it saw.
fn replay(answers: Vec<String>) -> (String, JoinHandle<Vec<(String, String)>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v2", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for answer in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                let read = stream.read(&mut buf).unwrap();
                raw.extend_from_slice(&buf[..read]);
                if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    let len: usize = String::from_utf8_lossy(&raw[..end])
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse().ok())?
                        })
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + len {
                        break end;
                    }
                }
            };
            let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
            let mut line = head.lines().next().unwrap_or_default().split(' ');
            let request = format!(
                "{} {}",
                line.next().unwrap_or_default(),
                line.next().unwrap_or_default()
            );
            seen.push((
                request,
                String::from_utf8_lossy(&raw[head_end + 4..]).into_owned(),
            ));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}",
                answer.len()
            )
            .unwrap();
        }
        seen
    });
    (base_url, handle)
}

fn ask(capture: &Value) -> AtomicQuoteRequest {
    let sent = json(capture, "quoteSent");
    assert_eq!(sent["direction"], "from");
    AtomicQuoteRequest {
        pair: sent["pair"].as_str().unwrap().to_owned(),
        amount: sent["amount"].as_u64().unwrap(),
        direction: AtomicAmountDirection::From,
    }
}

/// What a regtest wallet holding the captured asset would expect.
fn expect(capture: &Value) -> AtomicQuoteExpectations {
    let quote = json(capture, "quote");
    AtomicQuoteExpectations {
        asset_id: quote["assetId"].as_str().unwrap().to_owned(),
        bitcoin_network: "Regtest".to_owned(),
        proxy_url: Some(quote["offer"]["proxy_url"].as_str().unwrap().to_owned()),
        max_network_fee_sat: Some(2_000),
        max_service_fee: None,
        min_to_amount: None,
        max_from_amount: None,
        max_offer_ttl_secs: Some(600),
        // The real offers carry 1,000 sat per RGB output.
        max_rgb_output_sat: Some(1_000),
    }
}

/// The outpoints of an rgb-lib input list.
fn outpoints(inputs: &Value) -> Vec<AtomicOutpoint> {
    inputs
        .as_array()
        .unwrap()
        .iter()
        .map(|input| AtomicOutpoint {
            txid: input["outpoint"]["txid"].as_str().unwrap().to_owned(),
            vout: input["outpoint"]["vout"].as_u64().unwrap() as u32,
        })
        .collect()
}

/// The taker's wallet: the request's own inputs and a coin it did not spend.
fn wallet(request: &Value) -> Vec<AtomicOutpoint> {
    let mut coins = outpoints(&request["taker_inputs"]);
    coins.push(AtomicOutpoint {
        txid: "ee".repeat(32),
        vout: 7,
    });
    coins
}

fn now(capture: &Value) -> u64 {
    capture["now"].as_u64().unwrap()
}

#[tokio::test]
async fn a_real_atomic_swap_replays_through_the_client() {
    for pair in PAIRS {
        let capture = capture(pair);
        let (base_url, maker) = replay(vec![
            body(&capture, "quote"),
            body(&capture, "request"),
            body(&capture, "complete"),
            body(&capture, "status"),
        ]);
        let client = BoltzApiClientV2::new(base_url, None);

        let quote = client
            .atomic_quote(&ask(&capture), &expect(&capture), now(&capture))
            .await
            .unwrap_or_else(|e| panic!("{pair}: the real quote is refused: {e:?}"));
        assert_eq!(quote.offer, json(&capture, "quote")["offer"]);

        let request = json(&capture, "requestSent")["request"].clone();
        let proposed = client
            .atomic_request(&quote.id, &request, &wallet(&request))
            .await
            .unwrap_or_else(|e| panic!("{pair}: the real proposal is refused: {e:?}"));
        assert_eq!(proposed.status, "transaction.proposed");
        // The maker's coins come back for the wallet to check, and none of
        // them is one of the request's own inputs.
        let makers = &proposed.proposal["maker_inputs"];
        assert_eq!(
            proposed.maker_inputs.len(),
            makers.as_array().unwrap().len()
        );
        assert!(!proposed.maker_inputs.is_empty(), "{pair}");
        for (outpoint, raw) in proposed.maker_inputs.iter().zip(makers.as_array().unwrap()) {
            assert_eq!(outpoint.txid, raw["outpoint"]["txid"].as_str().unwrap());
            assert_eq!(
                u64::from(outpoint.vout),
                raw["outpoint"]["vout"].as_u64().unwrap()
            );
            for taker in request["taker_inputs"].as_array().unwrap() {
                assert_ne!(&raw["outpoint"], &taker["outpoint"], "{pair}");
            }
        }

        let completion = json(&capture, "completeSent")["completion"].clone();
        let done = client
            .atomic_complete(&quote.id, &completion)
            .await
            .unwrap_or_else(|e| panic!("{pair}: the real completion is refused: {e:?}"));

        let status = client.atomic_swap(&quote.id).await.unwrap();
        assert_eq!(status.status, AtomicSwapState::Claimed, "{pair}");
        assert!(status.status.is_settled());
        assert_eq!(status.txid.as_deref(), Some(done.txid.as_str()));

        // What the client sent is what the maker accepted.
        let seen = maker.join().unwrap();
        let id = &quote.id;
        let expected = [
            (
                "POST /v2/swap/atomic/quote".to_owned(),
                json(&capture, "quoteSent"),
            ),
            (
                format!("POST /v2/swap/atomic/{id}/request"),
                json(&capture, "requestSent"),
            ),
            (
                format!("POST /v2/swap/atomic/{id}/complete"),
                json(&capture, "completeSent"),
            ),
        ];
        for ((line, sent), (want_line, want)) in seen.iter().zip(&expected) {
            assert_eq!(line, want_line);
            assert_eq!(&serde_json::from_str::<Value>(sent).unwrap(), want);
        }
        assert_eq!(seen[3].0, format!("GET /v2/swap/atomic/{id}"));
    }
}

/// What a change to the offer is, and the change.
type Tamper = (&'static str, fn(&mut Value));

#[test]
fn a_real_quote_whose_offer_is_changed_is_refused() {
    let tamper: [Tamper; 6] = [
        ("maker gives less", |q| {
            let given = q["offer"]["maker_gives"]["amount"].as_u64().unwrap();
            q["offer"]["maker_gives"]["amount"] = (given - 1).into();
        }),
        ("maker takes more", |q| {
            let taken = q["offer"]["maker_receives"]["amount"].as_u64().unwrap();
            q["offer"]["maker_receives"]["amount"] = (taken + 1).into();
        }),
        ("higher miner fee", |q| {
            q["offer"]["network_fee_sat"] = 5_000.into();
        }),
        ("a platform fee", |q| {
            q["offer"]["platform_fee_sat"] = 1.into();
        }),
        ("a later expiry", |q| {
            let at = q["offer"]["expiration_timestamp"].as_u64().unwrap();
            q["offer"]["expiration_timestamp"] = (at + 3_600).into();
        }),
        ("another network", |q| {
            q["offer"]["bitcoin_network"] = "Signet".into();
        }),
    ];
    for pair in PAIRS {
        let capture = capture(pair);
        let real: AtomicQuote = serde_json::from_value(json(&capture, "quote")).unwrap();
        real.verify(&ask(&capture), &expect(&capture), now(&capture))
            .unwrap();
        for (what, change) in tamper {
            let mut quote = json(&capture, "quote");
            change(&mut quote);
            let quote: AtomicQuote = serde_json::from_value(quote).unwrap();
            let result = quote.verify(&ask(&capture), &expect(&capture), now(&capture));
            assert!(
                matches!(result, Err(Error::Protocol(_))),
                "{pair}, {what}: {result:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_real_proposal_for_another_request_is_refused() {
    for pair in PAIRS {
        let capture = capture(pair);
        let mut answer = json(&capture, "request");
        let outpoint = &mut answer["proposal"]["request"]["taker_inputs"][0]["outpoint"]["vout"];
        *outpoint = (outpoint.as_u64().unwrap() + 1).into();
        let (base_url, _maker) = replay(vec![answer.to_string()]);
        let client = BoltzApiClientV2::new(base_url, None);
        let id = json(&capture, "quote")["id"].as_str().unwrap().to_owned();
        let request = json(&capture, "requestSent")["request"].clone();
        let result = client
            .atomic_request(&id, &request, &wallet(&request))
            .await;
        assert!(
            matches!(result, Err(Error::Protocol(_))),
            "{pair}: {result:?}"
        );
    }
}

#[test]
fn the_real_status_parses() {
    for pair in PAIRS {
        let status: AtomicSwapStatus =
            serde_json::from_value(json(&capture(pair), "status")).unwrap();
        assert_eq!(status.pair, pair);
        assert_eq!(status.state, "settled");
    }
}

#[tokio::test]
async fn a_real_proposal_spending_another_of_the_takers_coins_is_refused() {
    // The real proposals, answered to a taker whose wallet also holds the
    // maker's first input: as if the maker had listed a coin of the taker's
    // that the request does not spend.
    for pair in PAIRS {
        let capture = capture(pair);
        let answer = json(&capture, "request");
        let request = json(&capture, "requestSent")["request"].clone();
        let mut taker = wallet(&request);
        let stolen = outpoints(&answer["proposal"]["maker_inputs"])[0].clone();
        taker.push(stolen.clone());
        let (base_url, _maker) = replay(vec![answer.to_string()]);
        let client = BoltzApiClientV2::new(base_url, None);
        let id = json(&capture, "quote")["id"].as_str().unwrap().to_owned();
        let error = client
            .atomic_request(&id, &request, &taker)
            .await
            .expect_err("the proposal must be refused");
        assert!(
            matches!(&error, Error::Protocol(m) if m.contains(&format!("taker's own coin {}:{}", stolen.txid, stolen.vout))),
            "{pair}: {error:?}"
        );
    }
}
