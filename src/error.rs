use std::fmt::{Display, Formatter};

use secp256k1_musig::musig;
use secp256k1_musig::scalar;
use serde_json::Value;

/// The Global Error enum. Encodes all possible internal library errors
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    #[cfg(feature = "electrum")]
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    Electrum(electrum_client::Error),
    #[cfg(feature = "esplora")]
    Esplora(String),
    Hex(String),
    Protocol(String),
    Key(bitcoin::key::ParsePublicKeyError),
    Address(String),
    Sighash(bitcoin::sighash::TaprootError),
    ElSighash(elements::sighash::Error),
    Secp(bitcoin::secp256k1::Error),
    /// A request that never produced a usable response: connection refused,
    /// DNS failure, a rejected certificate, a timeout, or a body this client
    /// could not read.
    ///
    /// Holds the error rather than its text because the actionable part is in
    /// that error's *cause*, not its own `Display`, which renders only its own
    /// layer — "error sending request for url (…)" — and says nothing about
    /// why. [`Error::source`](std::error::Error::source) reaches the rest, and
    /// [`Error::message_with_causes`] folds it into one string for a surface
    /// that can carry only text.
    ///
    /// Held directly rather than boxed, unlike [`Error::WebSocket`]:
    /// `reqwest::Error` is itself one pointer (`{ inner: Box<Inner> }`), so a
    /// second box would add an allocation per conversion and save nothing.
    HTTP(reqwest::Error),
    JSON(serde_json::Error),
    IO(std::io::Error),
    Bolt11(lightning_invoice::ParseOrSemanticError),
    LiquidEncode(elements::encode::Error),
    BitcoinEncode(bitcoin::consensus::encode::Error),
    Blind(String),
    ConfidentialTx(elements::ConfidentialTxOutError),
    BIP32(bitcoin::bip32::Error),
    BIP39(bip39::Error),
    BIP85(bip85_extended::Error),
    Hash(bitcoin::hashes::FromSliceError),
    Locktime(String),
    Url(url::ParseError),
    #[cfg(feature = "ws")]
    WebSocket(Box<tokio_tungstenite_wasm::Error>),
    Taproot(String),
    Musig2(String),
    /// A non-policy-asset Liquid spend cannot pay its transaction fee without
    /// at least one caller-provided policy-asset input.
    LiquidFeeAssetRequired,
    /// An RGB HTLC spend cannot pay its fee from the HTLC's own sats and
    /// still leave a colored output above the dust minimum: the caller's
    /// wallet has to add a BTC input (`RgbSpendFunding::CallerInputs`).
    RgbFeeInputRequired,
    Generic(String),
    HTTPStatusNotSuccess(reqwest::StatusCode, Value),
    /// A request the server answered with a success status, whose body did not
    /// deserialize into the expected type: the request worked and the two sides
    /// disagree on the schema.
    ///
    /// Distinct from `HTTPStatusNotSuccess`, which is the server rejecting the
    /// request. The payload describes the deserialization failure and never
    /// reproduces the body: a create response carries per-swap secrets and this
    /// error is routinely logged. See `BoltzApiClientV2::describe_parse_error`
    /// for what goes into it.
    HTTPResponseBodyInvalid(reqwest::StatusCode, String),
}

#[cfg(feature = "electrum")]
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl From<electrum_client::Error> for Error {
    fn from(value: electrum_client::Error) -> Self {
        Self::Electrum(value)
    }
}

impl From<bitcoin::hex::HexToBytesError> for Error {
    fn from(value: bitcoin::hex::HexToBytesError) -> Self {
        Self::Hex(value.to_string())
    }
}

impl From<bitcoin::key::ParsePublicKeyError> for Error {
    fn from(value: bitcoin::key::ParsePublicKeyError) -> Self {
        Self::Key(value)
    }
}

impl From<bitcoin::hex::HexToArrayError> for Error {
    fn from(value: bitcoin::hex::HexToArrayError) -> Self {
        Self::Hex(value.to_string())
    }
}

impl From<hex::FromHexError> for Error {
    fn from(value: hex::FromHexError) -> Self {
        Self::Hex(value.to_string())
    }
}

impl From<bitcoin::address::ParseError> for Error {
    fn from(value: bitcoin::address::ParseError) -> Self {
        Self::Address(value.to_string())
    }
}

impl From<elements::address::AddressError> for Error {
    fn from(value: elements::address::AddressError) -> Self {
        Self::Address(value.to_string())
    }
}

impl From<elements::sighash::Error> for Error {
    fn from(value: elements::sighash::Error) -> Self {
        Self::ElSighash(value)
    }
}

impl From<bitcoin::sighash::TaprootError> for Error {
    fn from(value: bitcoin::sighash::TaprootError) -> Self {
        Self::Sighash(value)
    }
}

impl From<bitcoin::secp256k1::Error> for Error {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        Self::Secp(value)
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::HTTP(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::JSON(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<lightning_invoice::ParseOrSemanticError> for Error {
    fn from(value: lightning_invoice::ParseOrSemanticError) -> Self {
        Self::Bolt11(value)
    }
}

impl From<elements::hex::Error> for Error {
    fn from(value: elements::hex::Error) -> Self {
        Self::Hex(value.to_string())
    }
}

impl From<elements::encode::Error> for Error {
    fn from(value: elements::encode::Error) -> Self {
        Self::LiquidEncode(value)
    }
}

impl From<elements::BlindError> for Error {
    fn from(value: elements::BlindError) -> Self {
        Self::Blind(value.to_string())
    }
}

impl From<elements::UnblindError> for Error {
    fn from(value: elements::UnblindError) -> Self {
        Self::Blind(value.to_string())
    }
}

impl From<elements::ConfidentialTxOutError> for Error {
    fn from(value: elements::ConfidentialTxOutError) -> Self {
        Self::ConfidentialTx(value)
    }
}

impl From<bitcoin::bip32::Error> for Error {
    fn from(value: bitcoin::bip32::Error) -> Self {
        Self::BIP32(value)
    }
}

impl From<bitcoin::hashes::FromSliceError> for Error {
    fn from(value: bitcoin::hashes::FromSliceError) -> Self {
        Self::Hash(value)
    }
}

impl From<bip39::Error> for Error {
    fn from(value: bip39::Error) -> Self {
        Self::BIP39(value)
    }
}

impl From<bip85_extended::Error> for Error {
    fn from(value: bip85_extended::Error) -> Self {
        Self::BIP85(value)
    }
}

impl From<bitcoin::absolute::ConversionError> for Error {
    fn from(value: bitcoin::absolute::ConversionError) -> Self {
        Self::Locktime(value.to_string())
    }
}

impl From<elements::locktime::Error> for Error {
    fn from(value: elements::locktime::Error) -> Self {
        Self::Locktime(value.to_string())
    }
}

impl From<url::ParseError> for Error {
    fn from(value: url::ParseError) -> Self {
        Self::Url(value)
    }
}

#[cfg(feature = "ws")]
impl From<tokio_tungstenite_wasm::Error> for Error {
    fn from(value: tokio_tungstenite_wasm::Error) -> Self {
        Self::WebSocket(value.into())
    }
}

impl From<bitcoin::taproot::TaprootError> for Error {
    fn from(value: bitcoin::taproot::TaprootError) -> Self {
        Self::Taproot(value.to_string())
    }
}

impl From<elements::taproot::TaprootError> for Error {
    fn from(value: elements::taproot::TaprootError) -> Self {
        Self::Taproot(value.to_string())
    }
}

impl From<elements::taproot::TaprootBuilderError> for Error {
    fn from(value: elements::taproot::TaprootBuilderError) -> Self {
        Self::Taproot(value.to_string())
    }
}

impl From<bitcoin::taproot::TaprootBuilderError> for Error {
    fn from(value: bitcoin::taproot::TaprootBuilderError) -> Self {
        Self::Taproot(value.to_string())
    }
}

impl From<bitcoin::consensus::encode::Error> for Error {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        Self::BitcoinEncode(value)
    }
}

impl From<musig::InvalidTweakErr> for Error {
    fn from(value: musig::InvalidTweakErr) -> Self {
        Self::Musig2(value.to_string())
    }
}

impl From<scalar::OutOfRangeError> for Error {
    fn from(value: scalar::OutOfRangeError) -> Self {
        Self::Musig2(value.to_string())
    }
}

impl From<musig::ParseError> for Error {
    fn from(value: musig::ParseError) -> Self {
        Self::Musig2(value.to_string())
    }
}

impl Error {
    // Returns the name of the enum variant as a string
    pub fn name(&self) -> String {
        match self {
            #[cfg(feature = "electrum")]
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            Error::Electrum(_) => "Electrum",
            #[cfg(feature = "esplora")]
            Error::Esplora(_) => "Esplora",
            Error::Hex(_) => "Hex",
            Error::Protocol(_) => "Protocol",
            Error::Key(_) => "Key",
            Error::Address(_) => "Address",
            Error::Sighash(_) => "Sighash",
            Error::ElSighash(_) => "Elements-Sighash",
            Error::Secp(_) => "Secp",
            Error::HTTP(_) => "HTTP",
            Error::JSON(_) => "JSON",
            Error::IO(_) => "IO",
            Error::Bolt11(_) => "Bolt11",
            Error::LiquidEncode(_) => "LiquidEncode",
            Error::BitcoinEncode(_) => "BitcoinEncode",
            Error::Blind(_) => "Blind",
            Error::ConfidentialTx(_) => "ConfidentialTx",
            Error::BIP32(_) => "BIP32",
            Error::BIP39(_) => "BIP39",
            Error::BIP85(_) => "BIP85",
            Error::Hash(_) => "Hash",
            Error::Locktime(_) => "Locktime",
            Error::Url(_) => "Url",
            #[cfg(feature = "ws")]
            Error::WebSocket(_) => "WebSocket",
            Error::Taproot(_) => "Taproot",
            Error::Musig2(_) => "Musig2",
            Error::LiquidFeeAssetRequired => "liquid_fee_asset_required",
            Error::RgbFeeInputRequired => "rgb_fee_input_required",
            Error::Generic(_) => "Generic",
            Error::HTTPStatusNotSuccess(_, _) => "HTTPStatusNotSuccess",
            Error::HTTPResponseBodyInvalid(_, _) => "HTTPResponseBodyInvalid",
        }
        .to_string()
    }

    // Returns the error message as a string
    pub fn message(&self) -> String {
        match self {
            #[cfg(feature = "electrum")]
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            Error::Electrum(e) => e.to_string(),
            #[cfg(feature = "esplora")]
            Error::Esplora(e) => e.clone(),
            Error::Hex(e) => e.clone(),
            Error::Protocol(e) => e.clone(),
            Error::Key(e) => e.to_string(),
            Error::Address(e) => e.clone(),
            Error::Sighash(e) => e.to_string(),
            Error::ElSighash(e) => e.to_string(),
            Error::Secp(e) => e.to_string(),
            Error::HTTP(e) => e.to_string(),
            Error::JSON(e) => e.to_string(),
            Error::IO(e) => e.to_string(),
            Error::Bolt11(e) => e.to_string(),
            Error::LiquidEncode(e) => e.to_string(),
            Error::BitcoinEncode(e) => e.to_string(),
            Error::Blind(e) => e.clone(),
            Error::ConfidentialTx(e) => e.to_string(),
            Error::BIP32(e) => e.to_string(),
            Error::BIP39(e) => e.to_string(),
            Error::BIP85(e) => e.to_string(),
            Error::Hash(e) => e.to_string(),
            Error::Locktime(e) => e.clone(),
            Error::Url(e) => e.to_string(),
            #[cfg(feature = "ws")]
            Error::WebSocket(e) => e.to_string(),
            Error::Taproot(e) => e.clone(),
            Error::Musig2(e) => e.clone(),
            Error::LiquidFeeAssetRequired => {
                "A caller-provided Liquid policy-asset input is required to pay fees".to_string()
            }
            Error::RgbFeeInputRequired => {
                "The RGB HTLC cannot pay this fee itself; a caller-provided BTC input is required"
                    .to_string()
            }
            Error::Generic(e) => e.clone(),
            Error::HTTPStatusNotSuccess(status, body) => {
                format!("HTTP Status Not Success: {status}, {body}")
            }
            Error::HTTPResponseBodyInvalid(status, description) => {
                format!("HTTP Response Body Invalid: {status}, {description}")
            }
        }
    }

    /// [`Error::message`] followed by every cause below it, `: ` apart.
    ///
    /// For a surface that can carry only a string — a UniFFI enum, a
    /// `js_sys::Error`, a log line — and so cannot walk
    /// [`source`](std::error::Error::source) itself. A Rust caller reporting a
    /// chain (`anyhow`, `eyre`, Sentry) already gets these layers separately
    /// and wants [`Error::message`] instead.
    ///
    /// [`Error::HTTP`] is what this exists for: its message is reqwest's own
    /// layer, and "connection refused" is a cause underneath it. Variants with
    /// no cause return exactly [`Error::message`].
    pub fn message_with_causes(&self) -> String {
        let mut rendered = self.message();
        let mut cause = std::error::Error::source(self);
        while let Some(next) = cause {
            rendered.push_str(": ");
            rendered.push_str(&next.to_string());
            cause = next.source();
        }
        rendered
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())?;
        Ok(())
    }
}

/// Without this, `?` cannot lift a library error into `Box<dyn Error>` or
/// `anyhow::Result` — the two things almost every consumer's `main` and every
/// wrapping error type are built on. `Display` alone is not enough for that.
///
/// `source` forwards *past* the wrapped error instead of returning it. This
/// enum's `Display` is [`Error::message`], which for a wrapped variant is that
/// error's own text — `JSON(serde_json::Error)` adds no context of its own — so
/// handing the same error back as the cause makes a reported chain print one
/// message twice, once as the error and again as what caused it. Forwarding is
/// the `#[error(transparent)]` semantic, and what [`std::io::Error`] itself does
/// with a custom payload: what a reader has not already seen is the wrapped
/// error's *own* cause. For `bitcoin::bip32::Error` that turns "base58 encoding
/// error" printed twice into "base58 encoding error" caused by "incorrect
/// checksum".
///
/// Most of the forwarding arms below answer `None` in practice, and that is the
/// correct answer rather than a gap: `serde_json::Error`, `url::ParseError`,
/// `secp256k1::Error`, `bip39::Error` and a syscall `io::Error` have no cause of
/// their own, and `electrum_client::Error`, `elements::sighash::Error` and
/// `ConfidentialTxOutError` have empty `std::error::Error` impls, while
/// `elements::encode::Error` overrides only the deprecated `cause`, which
/// `source`'s default does not delegate to. Where the old impl answered `Some`
/// for these it was handing back a pure duplicate, so nothing is lost. `HTTP`,
/// `BIP32` and `BitcoinEncode` are the three that genuinely go deeper, and
/// `HTTP` is the one where it matters operationally: reqwest renders only its
/// own layer, so "connection refused", "dns error" and "certificate verify
/// failed" all live below it.
///
/// Resist "fixing" the flat ones by reaching inside. `elements::encode::Error`'s
/// `Secp256k1zkp` payload, for instance, is rendered by that error's own
/// `Display` — returning it as the cause would reintroduce exactly the
/// duplication this forwarding removes.
///
/// The arms answering `None` unconditionally are enumerated rather than left to
/// a wildcard, so a new variant wrapping a concrete error has to make a choice
/// here — the way it already must in [`Error::name`] and [`Error::message`] —
/// instead of silently losing its cause. `Bolt11` and `BIP85` are there because
/// their upstream types do not implement `std::error::Error` at all, and the
/// `String` variants because whatever produced them was flattened at the
/// conversion site. Every one of them still carries its text in this error's own
/// `Display`.
///
/// The remaining `String` variants stay flattened for a structural reason, not
/// an accidental one: each is a *category* fed by several unrelated upstream
/// types — `Hex` by four, `Taproot` by four, `Address`, `Blind`, `Locktime` and
/// `Musig2` by two or three — so there is no one concrete error a variant could
/// hold. Giving them a chain needs a `Box<dyn Error>` payload, which trades the
/// typed access `HTTP` has for a cause chain; `HTTP` did not have to make that
/// trade, being fed by exactly one type.
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "electrum")]
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            Error::Electrum(e) => e.source(),
            Error::Key(e) => e.source(),
            Error::Sighash(e) => e.source(),
            Error::ElSighash(e) => e.source(),
            Error::Secp(e) => e.source(),
            Error::HTTP(e) => e.source(),
            Error::JSON(e) => e.source(),
            Error::IO(e) => e.source(),
            Error::LiquidEncode(e) => e.source(),
            Error::BitcoinEncode(e) => e.source(),
            Error::ConfidentialTx(e) => e.source(),
            Error::BIP32(e) => e.source(),
            Error::BIP39(e) => e.source(),
            Error::Hash(e) => e.source(),
            Error::Url(e) => e.source(),
            #[cfg(feature = "ws")]
            Error::WebSocket(e) => e.as_ref().source(),

            // Nothing below these to report.
            #[cfg(feature = "esplora")]
            Error::Esplora(_) => None,
            Error::Hex(_)
            | Error::Protocol(_)
            | Error::Address(_)
            | Error::Bolt11(_)
            | Error::Blind(_)
            | Error::BIP85(_)
            | Error::Locktime(_)
            | Error::Taproot(_)
            | Error::Musig2(_)
            | Error::LiquidFeeAssetRequired
            | Error::RgbFeeInputRequired
            | Error::Generic(_)
            | Error::HTTPStatusNotSuccess(_, _)
            | Error::HTTPResponseBodyInvalid(_, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use std::error::Error as StdError;
    use std::str::FromStr;

    /// Every message a reporter would print, outermost first.
    fn chain(error: &Error) -> Vec<String> {
        let mut rendered = vec![error.to_string()];
        let mut cause = StdError::source(error);
        while let Some(next) = cause {
            rendered.push(next.to_string());
            cause = next.source();
        }
        rendered
    }

    /// A `reqwest::Error` that has a cause and cost no I/O to produce.
    ///
    /// A URL this client cannot parse fails in the builder, before a socket is
    /// opened, and reqwest keeps the `url::ParseError` underneath as its
    /// source. That makes it the one reqwest error a unit test can construct
    /// offline while still having two layers, which is what the guards over
    /// [`wrapped_variants`] need.
    ///
    /// It is not the interesting *kind* — a refused connection is — but the
    /// kind does not change the plumbing: every one of them carries its detail
    /// in `source`, and [`a_refused_connection_reports_why_it_failed`]
    /// exercises one for real.
    fn unsent_request_error() -> reqwest::Error {
        reqwest::Client::new()
            .get("not a url")
            .build()
            .expect_err("an unparseable URL must fail in the builder")
    }

    /// One of each variant that wraps a concrete error, paired with the text
    /// that error renders on its own — captured before it was moved in.
    ///
    /// The pairing is the point. Asserting `to_string() == message()` would be
    /// true by construction, since `Display` *is* `message`; comparing against
    /// the wrapped error's own text is what lets the guard below fail if a
    /// `message` arm ever starts adding or dropping something.
    fn wrapped_variants() -> Vec<(Error, String)> {
        fn pair<E: std::fmt::Display>(inner: E, wrap: impl FnOnce(E) -> Error) -> (Error, String) {
            let rendered = inner.to_string();
            (wrap(inner), rendered)
        }
        use bitcoin::hashes::Hash as _;
        vec![
            pair(
                serde_json::from_str::<serde_json::Value>("{oops").unwrap_err(),
                Error::JSON,
            ),
            pair(url::Url::parse("not a url").unwrap_err(), Error::Url),
            pair(unsent_request_error(), Error::HTTP),
            pair(
                bitcoin::secp256k1::PublicKey::from_str("00").unwrap_err(),
                Error::Secp,
            ),
            pair(bitcoin::PublicKey::from_str("zz").unwrap_err(), Error::Key),
            pair(
                bitcoin::hashes::sha256::Hash::from_slice(&[1, 2, 3]).unwrap_err(),
                Error::Hash,
            ),
            pair(
                bitcoin::bip32::Xpriv::from_str("xprvBAD").unwrap_err(),
                Error::BIP32,
            ),
            pair(
                bip39::Mnemonic::from_str("not a valid mnemonic at all").unwrap_err(),
                Error::BIP39,
            ),
            pair(
                bitcoin::consensus::deserialize::<bitcoin::Transaction>(&[1, 2]).unwrap_err(),
                Error::BitcoinEncode,
            ),
            pair(
                elements::encode::deserialize::<elements::Transaction>(&[1, 2]).unwrap_err(),
                Error::LiquidEncode,
            ),
            pair(
                std::io::Error::from(std::io::ErrorKind::NotFound),
                Error::IO,
            ),
        ]
    }

    /// The constraint that rules out thinning `Display` to fix the duplication:
    /// callers printing `{e}` must keep seeing the wrapped error's own message,
    /// with nothing added and nothing dropped.
    ///
    /// Both binding crates depend on this holding. `bindings-wasm` reaches it
    /// two ways — `js_sys::Error::new(&e.message())` for core errors, and
    /// `arg_err`/`internal_err`, which are generic over `Display` and call
    /// `to_string()` — and `bindings` maps through `message()`. Neither can move
    /// while this passes.
    #[test]
    fn display_renders_the_wrapped_errors_own_message_and_nothing_else() {
        for (error, wrapped) in wrapped_variants() {
            assert_eq!(
                error.to_string(),
                wrapped,
                "{} renders something other than the error it wraps",
                error.name()
            );
            assert_eq!(
                error.message(),
                wrapped,
                "{} diverged from the error it wraps",
                error.name()
            );
        }
    }

    /// The regression this module exists for: `source()` used to hand back the
    /// same error whose text `Display` had just rendered, so a chain repeated
    /// one message. This fails for every variant against that implementation,
    /// which returned `Some` unconditionally.
    ///
    /// Necessarily conditional, though, and so no guard against the opposite
    /// mistake: most wrapped types have no cause of their own, and for those
    /// this says nothing. An arm that wrongly answered `None` where a cause
    /// exists is caught by `a_cause_below_the_wrapped_error_is_reached`, which
    /// covers the two variants that go deeper.
    #[test]
    fn a_wrapped_error_is_not_repeated_as_its_own_cause() {
        for (error, _) in wrapped_variants() {
            if let Some(cause) = StdError::source(&error) {
                assert_ne!(
                    error.to_string(),
                    cause.to_string(),
                    "{} reports its own message as its cause",
                    error.name()
                );
            }
        }
    }

    /// Forwarding is not merely "answer `None`": where the wrapped error has a
    /// cause of its own, the chain reaches it. Both of these hide a specific
    /// cause behind a generic message, which is what the duplicate stood in
    /// front of.
    ///
    /// The assertions deliberately do not pin exact depth or upstream wording.
    /// Every string here belongs to `bitcoin`/`base58ck` under a caret
    /// requirement, and a dependency bump that adds a layer or rewords one
    /// should not read as a regression in `source` on a PR that changed no
    /// code. What must hold is that the chain goes deeper than this enum and
    /// arrives somewhere `Display` did not already say.
    #[test]
    fn a_cause_below_the_wrapped_error_is_reached() {
        let bip32 = Error::BIP32(bitcoin::bip32::Xpriv::from_str("xprvBAD").unwrap_err());
        let rendered = chain(&bip32);
        assert!(
            rendered.len() >= 2,
            "no cause below the base58 error: {rendered:?}"
        );
        assert!(
            rendered.iter().skip(1).any(|m| m.contains("checksum")),
            "the chain never reaches the checksum failure: {rendered:?}"
        );

        let encode = Error::BitcoinEncode(
            bitcoin::consensus::deserialize::<bitcoin::Transaction>(&[1, 2]).unwrap_err(),
        );
        let rendered = chain(&encode);
        assert!(
            rendered.len() >= 2,
            "no cause below the encode error: {rendered:?}"
        );
        assert!(
            rendered.iter().skip(1).any(|m| m.contains("Eof")),
            "the chain never reaches the truncation: {rendered:?}"
        );
    }

    /// A rendered chain shows each layer once, for every wrapped variant.
    #[test]
    fn a_rendered_chain_prints_each_message_once() {
        for (error, _) in wrapped_variants() {
            let rendered = chain(&error);
            let mut seen = rendered.clone();
            seen.sort();
            seen.dedup();
            assert_eq!(
                seen.len(),
                rendered.len(),
                "{} renders a repeated message: {rendered:?}",
                error.name()
            );
        }
    }

    /// The deliberate `None`s. `Bolt11` and `BIP85` wrap a concrete error whose
    /// type does not implement the trait; the rest were flattened to a `String`
    /// at their conversion site. All of them keep their text in `Display`.
    #[test]
    fn flattened_and_unsupported_variants_report_no_cause() {
        for error in [
            // The two that wrap a concrete error whose type does not implement
            // the trait, so there is nothing this impl could forward to.
            Error::Bolt11(lightning_invoice::Bolt11Invoice::from_str("nonsense").unwrap_err()),
            Error::BIP85(bip85_extended::Error::InvalidWordCount(7)),
            Error::Generic("flattened".to_string()),
            Error::Hex("odd hex string length".to_string()),
            Error::Protocol("not a key".to_string()),
            Error::LiquidFeeAssetRequired,
            Error::RgbFeeInputRequired,
            Error::HTTPStatusNotSuccess(
                reqwest::StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "unauthorized"}),
            ),
        ] {
            assert!(
                StdError::source(&error).is_none(),
                "{} reported a cause it has no access to",
                error.name()
            );
            // The text still has to survive in this error's own Display.
            assert_eq!(chain(&error), vec![error.message()]);
            assert!(
                !error.message().is_empty(),
                "{} lost its text",
                error.name()
            );
        }
    }

    /// `HTTP` is the variant this matters most for, and the one with the least
    /// to show without it: reqwest's `Display` renders its own layer only —
    /// "error sending request for url (…)", or "builder error" here — and the
    /// reason is a cause underneath.
    ///
    /// Pinned structurally rather than by wording: the cause is reachable, and
    /// what it says is not already in this error's own message. The upstream
    /// text belongs to `reqwest`/`url` under caret requirements and is theirs
    /// to reword.
    #[test]
    fn a_failed_request_reaches_the_cause_reqwest_does_not_render() {
        let error = Error::HTTP(unsent_request_error());
        let rendered = chain(&error);

        assert!(
            rendered.len() >= 2,
            "no cause below the request error: {rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .skip(1)
                .any(|below| !error.message().contains(below.as_str())),
            "every layer only repeats what the message already said: {rendered:?}"
        );
    }

    /// What the variant holding the error buys beyond a chain: reqwest's own
    /// classification, which a `String` could not answer at all.
    ///
    /// `is_connect` walks reqwest's source chain internally, so it is also a
    /// second reader of the causes this change stopped discarding.
    #[test]
    fn a_failed_request_keeps_reqwests_own_classification() {
        let Error::HTTP(error) = Error::from(unsent_request_error()) else {
            panic!("From<reqwest::Error> must produce Error::HTTP");
        };
        assert!(error.is_builder(), "the builder failure was reclassified");
        // `is_connect` is `#[cfg(not(target_arch = "wasm32"))]` in reqwest, so
        // only this assertion is gated — `is_builder`, and the point of the
        // test, hold on every target. The predicate matches reqwest's exactly
        // rather than this module's usual `wasm32-unknown` pair, so it also
        // holds for `wasm32-wasip1`, where the method is equally absent.
        #[cfg(not(target_arch = "wasm32"))]
        assert!(
            !error.is_connect(),
            "a builder failure is not a connect one"
        );
    }

    /// The refused connection the variant exists for, end to end.
    ///
    /// Loopback on a port nothing listens on: no name to resolve, no external
    /// host, and a refusal that arrives immediately — nothing here can hang or
    /// depend on a third party. `is_connect` is asserted first so a failure for
    /// some *other* reason cannot pass as this one.
    ///
    /// Native-only, like every other test here that opens a socket: the wasm
    /// harness runs in a browser, where a fetch to a dead loopback port fails
    /// through the JS layer rather than as a connect error.
    ///
    /// `no_proxy` is load-bearing. `Client::new()` picks up `http_proxy` from
    /// the environment and has no loopback bypass, so on a runner or in a
    /// sandbox where that points at a proxy which *answers*, the send succeeds
    /// and this test fails on a `502` instead of the refusal it is about. A
    /// proxy that is itself dead would still pass, which is what makes the
    /// failure easy to miss. The timeout covers the other shape: a proxy or
    /// firewall that drops the connection rather than refusing it would
    /// otherwise hang to the harness limit.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[macros::async_test]
    async fn a_refused_connection_reports_why_it_failed() {
        let refused = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("a client with no proxy builds")
            .get("http://127.0.0.1:1/")
            .send()
            .await
            .expect_err("nothing listens on loopback port 1");
        assert!(
            refused.is_connect(),
            "not a connect failure, so this test proves nothing: {refused:?}"
        );

        let error = Error::from(refused);
        let rendered = chain(&error);
        assert!(
            rendered.len() >= 2,
            "the refusal is not reachable below reqwest's own layer: {rendered:?}"
        );
        assert!(
            error.message_with_causes().len() > error.message().len(),
            "a string-only surface still sees nothing but {:?}",
            error.message()
        );
    }

    /// [`Error::message_with_causes`] is the whole chain, `: ` apart, for the
    /// surfaces that cannot walk it — `bindings` maps `HTTP` through it.
    ///
    /// Stated against [`chain`] rather than against fixed text, so upstream
    /// rewording moves both sides together. What is pinned is the contract: it
    /// starts with [`Error::message`], it adds every layer below, and it adds
    /// nothing when there is no layer below.
    #[test]
    fn message_with_causes_folds_the_chain_and_message_still_does_not() {
        for (error, wrapped) in wrapped_variants() {
            let rendered = chain(&error);
            assert_eq!(
                error.message_with_causes(),
                rendered.join(": "),
                "{} folded its chain wrong",
                error.name()
            );
            assert!(
                error.message_with_causes().starts_with(&error.message()),
                "{} does not lead with its own message",
                error.name()
            );
            // The half of the contract `message` owns: folding is opt-in, and
            // does not leak into what `Display` renders.
            assert_eq!(error.message(), wrapped);
        }

        let flat = Error::Generic("nothing below this".to_string());
        assert_eq!(flat.message_with_causes(), flat.message());
    }

    /// The reason the impl exists at all: `?` lifting into the two result types
    /// almost every consumer's `main` is built on.
    #[test]
    fn the_error_lifts_through_question_mark() {
        fn boxed() -> Result<(), Box<dyn StdError>> {
            Err(Error::Generic("lifted".to_string()))?;
            Ok(())
        }
        fn with_anyhow() -> anyhow::Result<()> {
            Err(Error::Generic("lifted".to_string()))?;
            Ok(())
        }
        assert_eq!(boxed().unwrap_err().to_string(), "lifted");
        assert_eq!(with_anyhow().unwrap_err().to_string(), "lifted");
    }
}
