//! The skip-or-fail decision shared by every test that drives a live
//! third-party endpoint.
//!
//! Such a test has two failure modes that deserve opposite reactions, and
//! `assert!(result.is_ok())` cannot tell them apart:
//!
//! - **Nothing came back.** Nothing is learned, and failing here makes a third
//!   party's outage block every merge and release.
//! - **Something came back and this crate could not use it.** A schema or
//!   protocol regression — the only reason these tests exist.
//!
//! The error variant already carries that distinction, so reading it costs no
//! second request. An earlier version of the Boltz helper re-probed the host
//! over the network to decide; that doubled the wait against a dead host and
//! was itself what pushed the browser job over its limit.
//!
//! This module holds one copy of that decision for both transports. The two
//! call sites agree on what an outage is, and a new transport only has to say
//! which of *its* error variants mean the endpoint never answered.

use crate::error::Error;
use std::time::Duration;

/// How long a live third-party endpoint may take before it counts as absent.
///
/// Bounded, and comfortably under `wasm-bindgen-test`'s 20-second default per
/// test. An unbounded call is not portable: a host refusing connections returns
/// instantly on a developer machine, but the same dead host has consumed over
/// 60 seconds per call on the CI runner. Unbounded, that silently became a
/// browser-job timeout reported as "failed to detect test as having been run" —
/// no failing assertion, nothing to read.
///
/// Generous enough that a working endpoint answers well inside it, so the bound
/// decides "absent", never "slow but fine".
pub(crate) const LIVE_API_TIMEOUT: Duration = Duration::from_secs(10);

/// Unwrap a live third-party result, or skip the test if that endpoint never
/// answered.
///
/// Returns `None` after announcing a skip, so a caller with more to assert can
/// `let Some(value) = live(..) else { return };`.
pub(crate) fn live<T>(result: Result<T, Error>, endpoint: &str, what: &str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => match absent_because(&error) {
            // Announced, not silent: a run where every live test skipped must
            // not read like one where they all passed. With the cause, because
            // the transport's own layer only says a request was sent, and which
            // of refused / DNS / TLS / timed out it was is the whole content of
            // a skip line.
            //
            // Holding the error rather than its text is what makes the
            // classification available with no second request.
            Some(kind) => {
                let detail = error.message_with_causes();
                eprintln!("SKIPPED {what}: {endpoint} did not answer [{kind}] ({detail})");
                None
            }
            None => panic!("{what} failed against a responding {endpoint}: {error:?}"),
        },
    }
}

/// `Some(kind)` when the endpoint never gave this crate something to judge, and
/// the test should skip; `None` when it answered and the answer is the test's
/// business, so the test should fail.
///
/// `kind` is named in the skip line rather than flattened into every other
/// reason a host was unreachable, so an unexpected shape shows up in the log.
fn absent_because(error: &Error) -> Option<&'static str> {
    match error {
        // `Error::HTTP` is this crate's `From<reqwest::Error>`. A rejected
        // status becomes `HTTPStatusNotSuccess` and a body that will not
        // deserialize becomes `HTTPResponseBodyInvalid` or `JSON`, so this
        // variant is a transport failure: refused, DNS, TLS, the timeout above
        // — or, as the doc on the variant says explicitly, a body this client
        // could not read, such as a mid-body reset.
        //
        // All of those skip. A third party resetting mid-response is that party
        // having a bad minute, not the schema or protocol regression these
        // tests exist to detect, and failing on it is the "outage blocks every
        // merge" case above.
        Error::HTTP(e) if e.is_connect() => Some("connect"),
        Error::HTTP(e) if e.is_timeout() => Some("timeout"),
        Error::HTTP(e) if e.is_body() || e.is_decode() => Some("body"),
        Error::HTTP(_) => Some("other"),
        #[cfg(feature = "electrum")]
        Error::Electrum(e) => electrum_absent_because(e),
        _ => None,
    }
}

/// The same question for Electrum, whose error type draws the line in a
/// different place than `reqwest`'s.
///
/// Everything that reached the socket layer and failed there is absent. What is
/// left — a server-sent protocol error, a response that would not deserialize,
/// and this crate's own misuse of the client — is an answer, and an answer is
/// what these tests exist to judge.
#[cfg(feature = "electrum")]
fn electrum_absent_because(error: &electrum_client::Error) -> Option<&'static str> {
    use electrum_client::Error as Electrum;
    match error {
        Electrum::IOError(e) => Some(io_kind(e)),
        Electrum::SharedIOError(e) => Some(io_kind(e)),
        // One entry per resolved address the client tried. Absent only if no
        // attempt got an answer — one usable-looking reply among them is still
        // the regression these tests watch for. The first attempt's kind names
        // the skip and the detail printed beside it lists them all, so nothing
        // is lost when the addresses failed for different reasons. An empty
        // list cannot happen and is not worth inventing a kind for, so it falls
        // through to a failure.
        Electrum::AllAttemptsErrored(errors) => {
            let mut kinds = errors.iter().map(electrum_absent_because);
            let first = kinds.next()??;
            kinds.all(|kind| kind.is_some()).then_some(first)
        }
        // `MissingDomain`, `InvalidDNSNameError` and `CouldNotCreateConnection`
        // are settled before a packet leaves: a URL or TLS setup this crate got
        // wrong, which fails identically on a perfectly healthy network. Those
        // belong in the failing half with the parse errors, not the skipping
        // half — a permanent bug must not hide behind an outage.
        _ => None,
    }
}

/// Electrum hands back a bare [`std::io::Error`] for everything from a refused
/// connection to a failed TLS handshake, so the kind is all there is to sort by.
///
/// Every one of them skips; this only decides what the skip line calls it.
#[cfg(feature = "electrum")]
fn io_kind(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match error.kind() {
        // The read/write deadline `build_client` sets, and the connect deadline
        // under it.
        ErrorKind::TimedOut | ErrorKind::WouldBlock => "timeout",
        ErrorKind::ConnectionRefused
        | ErrorKind::ConnectionAborted
        | ErrorKind::ConnectionReset
        | ErrorKind::NotConnected
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof
        | ErrorKind::AddrNotAvailable
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkUnreachable
        | ErrorKind::NetworkDown => "connect",
        // rustls reports a handshake failure — an untrusted or mismatched
        // certificate included — by wrapping its own error at this kind.
        ErrorKind::InvalidData => "tls",
        // Name resolution lands here: `getaddrinfo` failing is uncategorized on
        // Linux, and `ErrorKind::Uncategorized` cannot be named on stable. The
        // detail printed beside the kind still says "failed to lookup address
        // information", so the skip line stays readable without this guessing.
        _ => "other",
    }
}
