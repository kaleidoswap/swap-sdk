//! KaleidoSwap partner attribution: the organization API key and the client
//! that carries it.
//!
//! A partner organization gets a key from the KaleidoSwap Platform panel and
//! configures it here. The maker introspects the key, resolves it to an
//! organization, and stores that organization with the swap, so the partner can
//! later see statistics for the swaps they originated.
//!
//! The key is **attribution only**. It answers *"which partner organization
//! created this swap?"* and nothing else — it authorizes no claim, no refund, no
//! fund movement, no panel access and no administrative operation. The per-swap
//! credential the maker issues as `swapAuth` is what answers *"is this caller
//! allowed to decide the outcome of this specific swap?"*, and the two stay
//! separate: see [`SWAP_AUTH_HEADER`].
//!
//! [`SWAP_AUTH_HEADER`]: crate::boltz::SWAP_AUTH_HEADER
//! [`BoltzApiClientV2`]: crate::boltz::BoltzApiClientV2
//!
//! ```no_run
//! use kaleidorg_swap_sdk::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};
//!
//! # async fn run() -> Result<(), kaleidorg_swap_sdk::error::Error> {
//! let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
//!     maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
//!     api_key: std::env::var("KALEIDOSWAP_API_KEY").unwrap().parse::<ApiKey>()?,
//!     timeout: None,
//! })?;
//!
//! // Every maker route is reachable through the deref to the generic client.
//! let pairs = client.get_submarine_pairs().await?;
//! # let _ = pairs;
//! # Ok(())
//! # }
//! ```
//!
//! # Browsers are out of scope
//!
//! A key configured here is a permanent organization credential. Embedded in
//! browser JavaScript it is visible to every visitor, who can then attribute
//! their swaps to — or exhaust the rate limits of — an organization that is not
//! theirs. **This release supports server and native integrations only.** Build
//! the key into a backend that talks to the maker on the browser's behalf, and
//! leave the browser bundle on the unauthenticated
//! [`BoltzApiClientV2`].
//!
//! # Organization keys: attribution for client-side software
//!
//! An app, browser extension or web page cannot keep an API key secret, so it
//! attributes its swaps with an [`OrgKey`] instead: the organization's x-only
//! secp256k1 public key, registered in the partner panel with a signature that
//! proves the organization holds the private half. [`KaleidoOrgClient`] sends it
//! in [`ORG_KEY_HEADER`] on the swap-creation routes and nowhere else.
//!
//! The org key is a **label, not a credential**. Anyone can copy it, so it
//! buys the caller nothing: no rate-limit budget of its own, no discount, no
//! access in the maker's partner-keys-only mode. That is why it is safe to ship
//! in a client, and why it cannot replace an [`ApiKey`] on a server that needs
//! one of those. The private key is used only to claim a revshare payout on
//! the partner's own machine; nothing in this SDK asks for it.
//!
//! ```no_run
//! use kaleidorg_swap_sdk::kaleido::{KaleidoOrgClient, KaleidoOrgClientOptions, OrgKey};
//!
//! # fn run() -> Result<(), kaleidorg_swap_sdk::error::Error> {
//! let client = KaleidoOrgClient::new(KaleidoOrgClientOptions {
//!     maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
//!     org_key: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
//!         .parse::<OrgKey>()?,
//!     timeout: None,
//! })?;
//! # let _ = client;
//! # Ok(())
//! # }
//! ```

use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use std::time::Duration;

use bitcoin::XOnlyPublicKey;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

use crate::error::Error;
use crate::swaps::boltz::BoltzApiClientV2;

/// The header the organization API key travels in, as a `Bearer` credential.
///
/// The standard `Authorization` header on purpose, and not a custom one: HTTP
/// clients strip `Authorization` when a redirect leaves the host it was
/// addressed to, while a custom header rides along to whatever the `Location`
/// names. `X-Swap-Auth` had to be defended against that with a redirect policy
/// of its own; this one is largely covered by the client's own behaviour.
///
/// *Largely*, not entirely. `reqwest` compares host and effective port and
/// ignores the scheme, so `https://h` → `http://h:443` keeps the header and
/// sends the key in the clear. The SDK cannot stop that on a redirect-following
/// client, but it does report it: the redirect check picks its advice from
/// reqwest's rule rather than from the stricter one the SDK uses to decide what
/// counts as the maker, so a scheme-only hop says *revoke the key*.
pub const API_KEY_HEADER: &str = "Authorization";

/// The prefix every KaleidoSwap organization API key carries.
pub const API_KEY_PREFIX: &str = "kld";

/// The environment a key is scoped to.
///
/// Platform issues a key for exactly one, and it is enforced on the far side:
/// introspection compares the key's environment against the calling maker's, and
/// the maker compares the introspected environment against its own configuration.
/// A `Test` key sent to a production maker is rejected, not quietly accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiKeyEnvironment {
    /// `kld_test_…` — signet and staging.
    Test,
    /// `kld_live_…` — mainnet and production.
    Live,
}

impl ApiKeyEnvironment {
    /// The segment as it appears in the key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Live => "live",
        }
    }
}

impl Display for ApiKeyEnvironment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A parsed KaleidoSwap organization API key: `kld_<env>_<key_id>_<secret>`.
///
/// Parsing is structural, not a validity check — only the Platform knows whether
/// a well-formed key is active, and only the maker's introspection call can say
/// so. What parsing buys is that a value which cannot possibly be a key fails
/// here, naming itself, instead of arriving as a `401` that reads exactly like a
/// revoked key or a suspended organization.
///
/// The secret never leaves this type: there is no accessor for it, no `Display`,
/// no `Serialize`, and [`Debug`] renders [`Self::redacted`].
///
/// It is held in [`Zeroizing`], which wipes the stored copy and the per-request
/// buffer on drop. That bounds the exposure rather than ending it: the
/// `HeaderValue` each request carries, the buffer hyper encodes it into, and the
/// string the caller handed to [`Self::parse`] are all outside this type and
/// none of them are wiped. Treat a process that has sent one authenticated
/// request as a process with the key in its heap.
#[derive(Clone)]
pub struct ApiKey {
    environment: ApiKeyEnvironment,
    key_id: String,
    secret: Zeroizing<String>,
}

impl ApiKey {
    /// The longest key this will parse.
    ///
    /// Nothing in the format is variable enough to approach this; the cap is
    /// here so an accidentally-passed file or JSON document is rejected as a
    /// malformed key rather than assembled into a header.
    const MAX_LEN: usize = 512;

    /// Parse a key, rejecting anything that cannot reach the maker as one.
    ///
    /// Surrounding ASCII whitespace is trimmed: `KALEIDOSWAP_API_KEY` read from
    /// a file or a shell heredoc routinely carries a trailing newline, and no
    /// byte of a key is whitespace, so trimming cannot turn one valid key into
    /// another. Whitespace *inside* the value is still an error.
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let raw = raw.trim_matches(|c: char| c.is_ascii_whitespace());

        if raw.is_empty() {
            return Err(Self::malformed("it is empty"));
        }
        if raw.len() > Self::MAX_LEN {
            return Err(Self::malformed(&format!(
                "it is {} bytes long, and no key exceeds {}",
                raw.len(),
                Self::MAX_LEN
            )));
        }

        let rest = raw
            .strip_prefix(API_KEY_PREFIX)
            .and_then(|rest| rest.strip_prefix('_'));
        let Some(rest) = rest else {
            return Err(Self::malformed(&format!(
                "it does not start with `{API_KEY_PREFIX}_`"
            )));
        };

        let Some((environment, rest)) = rest.split_once('_') else {
            return Err(Self::malformed(
                "it has no environment segment — expected `kld_test_…` or `kld_live_…`",
            ));
        };
        let environment = match environment {
            "test" => ApiKeyEnvironment::Test,
            "live" => ApiKeyEnvironment::Live,
            other => {
                return Err(Self::malformed(&format!(
                    "`{other}` is not a key environment — expected `test` or `live`"
                )))
            }
        };

        // `split_once` and not a full split: the key id never contains an
        // underscore, so everything after the third one is the secret — which
        // may itself be base64url, where `_` is a perfectly ordinary byte.
        //
        // The key id is the half this assumption rests on. Were Platform ever to
        // issue one containing `_`, the split would land early: the wire value
        // still reconstructs byte-for-byte (the same separators go back in, and
        // `a_key_parses_into_its_parts_and_the_secret_keeps_its_underscores`
        // pins that), so swaps would keep working — but `key_id()` and
        // `redacted()` would report a truncated id, which is the value the docs
        // tell partners to match against the panel. The charset check below
        // pins the assumption from this side.
        let Some((key_id, secret)) = rest.split_once('_') else {
            return Err(Self::malformed(
                "it has no secret segment — expected `kld_<env>_<key_id>_<secret>`",
            ));
        };
        if key_id.is_empty() {
            return Err(Self::malformed("its key id is empty"));
        }
        if !key_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Self::malformed(
                "its key id has a character outside `A-Za-z0-9-`",
            ));
        }
        if secret.is_empty() {
            return Err(Self::malformed("its secret is empty"));
        }
        // Exactly the bytes a header value may carry. Anything else — a newline
        // spliced in, a stray tab, a non-ASCII character from a smart-quoting
        // editor — would be refused by `HeaderValue` at send time as a bare
        // failure with no name attached to it.
        if !secret.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            return Err(Self::malformed(
                "its secret has a character that cannot travel in a header",
            ));
        }

        Ok(Self {
            environment,
            key_id: key_id.to_string(),
            secret: Zeroizing::new(secret.to_string()),
        })
    }

    fn malformed(why: &str) -> Error {
        Error::Protocol(format!(
            "not a KaleidoSwap organization API key: {why}. Keys look like \
             `kld_test_<key_id>_<secret>` and are issued in the partner panel"
        ))
    }

    /// The environment the key is scoped to.
    pub fn environment(&self) -> ApiKeyEnvironment {
        self.environment
    }

    /// The key's public identifier — the segment Platform also shows in the
    /// panel, safe to log and to name in a support request.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// The key with its secret replaced, for logs and error messages.
    pub fn redacted(&self) -> String {
        format!(
            "{API_KEY_PREFIX}_{}_{}_<redacted>",
            self.environment, self.key_id
        )
    }

    /// The `Authorization` value this key sends, marked sensitive.
    ///
    /// [`Self::parse`] already rejected every byte a header value cannot carry,
    /// so the conversion cannot fail for a key that exists — it is still
    /// handled rather than unwrapped, because the alternative to an error here
    /// is a panic inside somebody's swap.
    pub(crate) fn bearer_header_value(&self) -> Result<HeaderValue, Error> {
        let bearer = Zeroizing::new(format!(
            "Bearer {API_KEY_PREFIX}_{}_{}_{}",
            self.environment,
            self.key_id,
            self.secret.as_str()
        ));
        let mut value = HeaderValue::from_str(&bearer).map_err(|_| {
            Error::Protocol(format!(
                "organization API key {} cannot be sent as an {API_KEY_HEADER} value",
                self.redacted()
            ))
        })?;
        // Keeps the key out of hyper's header dumps and, over HTTP/2, out of the
        // HPACK dynamic table that a connection-level observer — or a later
        // request sharing the connection — could otherwise index it into.
        value.set_sensitive(true);
        Ok(value)
    }
}

impl FromStr for ApiKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for ApiKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ApiKey {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}

/// Renders [`ApiKey::redacted`]. The derive would print the secret, and a client
/// holding a key is exactly the kind of value a caller reaches for `{:?}` on.
impl Debug for ApiKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ApiKey").field(&self.redacted()).finish()
    }
}

/// The header an [`OrgKey`] travels in: 64 lowercase hex, no scheme.
///
/// A custom header and not `Authorization`, because the value is not a
/// credential. Reusing `Authorization` would make the maker read it as a bearer
/// token, and would make every HTTP stack on the way treat a public label as a
/// secret to strip on redirect.
pub const ORG_KEY_HEADER: &str = "X-Kaleido-Org-Key";

/// The header the maker answers every swap creation with: `none`, `api_key` or
/// `org_key`, naming the attribution the swap was actually stored with.
///
/// A maker that has organization keys switched off ignores [`ORG_KEY_HEADER`]
/// and answers `none`; [`KaleidoOrgClient`] logs a warning when that happens,
/// so a dropped attribution shows up in the integrator's logs rather than as a
/// gap in their statistics weeks later. In a browser the header is readable
/// only where the maker's edge exposes it, and its absence is not reported.
pub const ATTRIBUTION_HEADER: &str = "X-Kaleido-Attribution";

/// A KaleidoSwap organization key: the BIP-340 x-only public key a partner
/// registered in the panel, as 32 bytes.
///
/// The opposite of [`ApiKey`] on purpose. It is public, so it prints, compares,
/// serializes and goes in logs and config files like any other identifier, and
/// it is sent as an ordinary header value — not marked sensitive, not wiped.
///
/// Parsing checks that the value is a point on the curve, not that anyone
/// registered it: a well-formed but unregistered key still attributes the swap
/// at the maker, and Platform files it as unregistered until the organization
/// registers it. What parsing buys is that a typo fails here, naming itself,
/// instead of reaching the maker as a `400 org_key_malformed`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrgKey(XOnlyPublicKey);

impl OrgKey {
    /// Hex characters in a key.
    const HEX_LEN: usize = 64;

    /// Parse a key from 64 hex characters, either case.
    ///
    /// Surrounding ASCII whitespace is trimmed, for the same reason as
    /// [`ApiKey::parse`]: a value read from a file or an env var routinely
    /// carries a trailing newline.
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let raw = raw.trim_matches(|c: char| c.is_ascii_whitespace());
        if raw.len() != Self::HEX_LEN {
            return Err(Self::malformed(&format!(
                "it is {} characters long, and a key is {}",
                raw.chars().count(),
                Self::HEX_LEN
            )));
        }
        if !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Self::malformed("it has a character that is not hex"));
        }
        // Lowercased first so the maker, Platform and this type agree on one
        // spelling of one key: the registry stores lowercase, and an uppercase
        // key that parsed here but was compared as text elsewhere would read
        // as a different organization.
        XOnlyPublicKey::from_str(&raw.to_ascii_lowercase())
            .map(Self)
            .map_err(|_| Self::malformed("it is not a point on the secp256k1 curve"))
    }

    fn malformed(why: &str) -> Error {
        Error::Protocol(format!(
            "not a KaleidoSwap organization key: {why}. An organization key is the \
             64-hex x-only public key registered in the partner panel"
        ))
    }

    /// The key as a secp256k1 x-only public key.
    pub fn x_only_public_key(&self) -> XOnlyPublicKey {
        self.0
    }

    /// The [`ORG_KEY_HEADER`] value this key sends.
    pub(crate) fn header_value(&self) -> HeaderValue {
        HeaderValue::from_str(&self.to_string())
            .expect("64 lowercase hex characters are a valid header value")
    }
}

impl From<XOnlyPublicKey> for OrgKey {
    fn from(key: XOnlyPublicKey) -> Self {
        Self(key)
    }
}

impl FromStr for OrgKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for OrgKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for OrgKey {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}

/// 64 lowercase hex, exactly as the header carries it.
impl Display for OrgKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// The hex, rather than the derive: `XOnlyPublicKey`'s own `Debug` prints the
/// library's internal 64-byte representation, which matches nothing a partner
/// can find in the panel.
impl Debug for OrgKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("OrgKey").field(&self.to_string()).finish()
    }
}

impl Serialize for OrgKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for OrgKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(|e| serde::de::Error::custom(e.message()))
    }
}

/// How to build a [`KaleidoMakerClient`].
#[derive(Debug)]
pub struct KaleidoMakerClientOptions {
    /// The maker's `/v2` base URL — and the only origin the key is ever sent to.
    pub maker_url: String,
    /// The organization API key from the partner panel.
    pub api_key: ApiKey,
    /// Per-request timeout. `None` leaves it to the HTTP stack.
    pub timeout: Option<Duration>,
}

/// A [`BoltzApiClientV2`] that attributes the swaps it creates to a partner
/// organization.
///
/// A separate type rather than a flag on the generic client, because the key
/// changes what the client may be pointed at: it is bound to one origin at
/// construction and refuses to travel anywhere else, so a
/// [`BoltzApiClientV2`] built for Boltz, Esplora or a second maker cannot end up
/// carrying it. See the [module docs][self] for what the key does and does not
/// authorize, and why browsers are out of scope.
///
/// Every maker route is available through [`Deref`], so this is a drop-in for the
/// generic client:
///
/// ```no_run
/// # use kaleidorg_swap_sdk::kaleido::KaleidoMakerClient;
/// # async fn run(client: KaleidoMakerClient) -> Result<(), kaleidorg_swap_sdk::error::Error> {
/// let height = client.get_height().await?;
/// # let _ = height;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct KaleidoMakerClient {
    inner: BoltzApiClientV2,
    /// The same key the inner client carries. Held here so the accessors below
    /// are total: reaching for it through `inner.api_key()` would be an
    /// `Option` this type can never see as `None`, and the `expect` closing
    /// that gap is a panic path in a library the moment a third constructor is
    /// added without one.
    api_key: ApiKey,
}

impl KaleidoMakerClient {
    /// Build a client that sends `options.api_key` to `options.maker_url`.
    ///
    /// Fails when the URL cannot carry a bearer credential safely: it must be
    /// `https`, or `http` to a loopback host for the regtest harness.
    pub fn new(options: KaleidoMakerClientOptions) -> Result<Self, Error> {
        let KaleidoMakerClientOptions {
            maker_url,
            api_key,
            timeout,
        } = options;
        Self::validate_maker_url(&maker_url)?;
        Ok(Self {
            inner: BoltzApiClientV2::new(maker_url, timeout).with_api_key(api_key.clone()),
            api_key,
        })
    }

    /// [`Self::new`] over a caller-supplied `reqwest::ClientBuilder`, keeping the
    /// proxy, TLS and pool configuration set on it.
    ///
    /// A *builder* and not a built `reqwest::Client`, because the redirect policy
    /// is the one setting a caller does not get to choose here. A
    /// redirect-following client leaks on two counts: [`SWAP_AUTH_HEADER`],
    /// carried on a chain re-quote, is a custom header and follows the hop
    /// unconditionally; and `reqwest` keeps `Authorization` when only the scheme
    /// changes, so an `https` maker answering `302 http://same-host:443/…`
    /// re-sends the organization key in the clear.
    ///
    /// Neither can be caught afterwards. A [`reqwest::Client`] does not report
    /// its policy, and a [`reqwest::Response`] carries only the URL the chain
    /// *ended* at — no hop list, no `redirected` flag — so a chain that detoured
    /// through another host and came back is indistinguishable from no redirect
    /// at all. Taking the configuration before it is frozen is what makes the
    /// guarantee structural instead of advisory: this applies
    /// [`reqwest::redirect::Policy::none`] itself and leaves every other setting
    /// on the builder untouched.
    ///
    /// The browser is the one place this cannot help. `fetch` owns redirect
    /// handling and wasm-bindgen can set no policy on it, so there the hop is
    /// reported after the fact by
    /// [`BoltzApiClientV2::reject_credential_leaking_redirect`] rather than
    /// declined — see the [module docs][self] for why browsers are out of scope.
    ///
    /// [`SWAP_AUTH_HEADER`]: crate::boltz::SWAP_AUTH_HEADER
    /// [`BoltzApiClientV2::reject_credential_leaking_redirect`]: crate::boltz::BoltzApiClientV2
    pub fn with_client_builder(
        options: KaleidoMakerClientOptions,
        http_client: reqwest::ClientBuilder,
    ) -> Result<Self, Error> {
        let KaleidoMakerClientOptions {
            maker_url,
            api_key,
            timeout,
        } = options;
        Self::validate_maker_url(&maker_url)?;
        // No `redirect` on the wasm builder to call: `fetch` owns redirects and
        // exposes no policy, which is why the after-the-fact check exists.
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let http_client = http_client.redirect(reqwest::redirect::Policy::none());
        let http_client = http_client.build()?;
        Ok(Self {
            inner: BoltzApiClientV2::with_client(maker_url, http_client, timeout)
                .with_api_key(api_key.clone()),
            api_key,
        })
    }

    /// Reject a maker URL a key must not be sent to.
    ///
    /// `https`, or `http` to a loopback host. A bearer credential over plain
    /// HTTP is readable by anything on the path, and a permanent organization
    /// key read once is a key that has to be revoked — so a plain-HTTP maker
    /// fails here rather than after the key is already on the wire. Loopback is
    /// exempt because that is the regtest harness, where the "network" is a
    /// socket on the same machine.
    ///
    /// A URL carrying userinfo is refused too, and that one is not about
    /// eavesdropping. `reqwest` turns `https://user:pw@host/v2` into an
    /// `Authorization: Basic …` header of its own, and `RequestBuilder::header`
    /// *appends* — so the request would go out with two `Authorization` headers
    /// and the maker, reading the first, would never see the key. In the maker's
    /// optional mode that is a swap silently recorded as anonymous, which is the
    /// one failure this whole feature is supposed to make impossible; in its
    /// required mode it is a `401` with nothing in it to diagnose. Nothing
    /// legitimate needs credentials in the URL, so this fails loudly instead.
    fn validate_maker_url(maker_url: &str) -> Result<(), Error> {
        let url = reqwest::Url::parse(maker_url)?;
        let Some(host) = url.host_str() else {
            return Err(Error::Protocol(format!(
                "maker URL {maker_url} names no host — the organization API key is \
                 bound to one origin and there is nothing here to bind it to"
            )));
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Error::Protocol(format!(
                "maker URL for {host} carries a username or password — those become \
                 an Authorization header of their own, which would displace the \
                 organization API key and leave the swap unattributed. Put the \
                 credentials in your own reqwest client instead"
            )));
        }
        match url.scheme() {
            "https" => Ok(()),
            "http" if is_loopback(host) => Ok(()),
            "http" => Err(Error::Protocol(format!(
                "refusing to send an organization API key to {maker_url} over plain \
                 HTTP — anything on the path could read it, and the key is permanent \
                 until revoked. Use https, or the unauthenticated BoltzApiClientV2"
            ))),
            scheme => Err(Error::Protocol(format!(
                "maker URL {maker_url} is {scheme}, and the SDK speaks http(s)"
            ))),
        }
    }

    /// The environment the configured key is scoped to.
    pub fn api_key_environment(&self) -> ApiKeyEnvironment {
        self.api_key_ref().environment()
    }

    /// The configured key's public identifier.
    pub fn api_key_id(&self) -> &str {
        self.api_key_ref().key_id()
    }

    /// The generic client underneath, for code that takes one by reference.
    pub fn client(&self) -> &BoltzApiClientV2 {
        &self.inner
    }

    /// Take the generic client out. It keeps the key and the origin it is bound
    /// to; nothing about the credential is dropped on the way.
    pub fn into_inner(self) -> BoltzApiClientV2 {
        self.inner
    }

    fn api_key_ref(&self) -> &ApiKey {
        &self.api_key
    }
}

impl Deref for KaleidoMakerClient {
    type Target = BoltzApiClientV2;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// How to build a [`KaleidoOrgClient`].
#[derive(Debug, Clone)]
pub struct KaleidoOrgClientOptions {
    /// The maker's `/v2` base URL — the only origin the key is sent to.
    pub maker_url: String,
    /// The organization key registered in the partner panel.
    pub org_key: OrgKey,
    /// Per-request timeout. `None` leaves it to the HTTP stack.
    pub timeout: Option<Duration>,
}

/// A [`BoltzApiClientV2`] that attributes the swaps it creates to a partner
/// organization by its public [`OrgKey`]. Safe to use in client-side software;
/// see the [module docs][self].
///
/// It sends [`ORG_KEY_HEADER`] on the swap-creation routes —
/// `POST /v2/swap/{submarine,reverse,chain}` and the corridor's `POST /v1/swap`
/// — and on nothing else: a status read, a claim or a pair list has nothing to
/// attribute, and a label sent everywhere is a label every response cache and
/// access log along the way collects for no reason.
///
/// A separate type from [`KaleidoMakerClient`] rather than a second key on it,
/// so one client never carries both. The maker refuses a request with both
/// (`400 attribution_conflict`).
#[derive(Debug, Clone)]
pub struct KaleidoOrgClient {
    inner: BoltzApiClientV2,
    org_key: OrgKey,
}

impl KaleidoOrgClient {
    /// Build a client that attributes its swaps on `options.maker_url` to
    /// `options.org_key`.
    ///
    /// Fails when the URL is not `https`, or `http` to a loopback host. The key
    /// is public, so this is not about secrecy: over plain HTTP anything on the
    /// path can strip or replace the header, and the swap is attributed to
    /// somebody else or to nobody with no sign of it on either end.
    pub fn new(options: KaleidoOrgClientOptions) -> Result<Self, Error> {
        let KaleidoOrgClientOptions {
            maker_url,
            org_key,
            timeout,
        } = options;
        Self::validate_maker_url(&maker_url)?;
        Ok(Self {
            inner: BoltzApiClientV2::new(maker_url, timeout).with_org_key(org_key),
            org_key,
        })
    }

    /// [`Self::new`] over a caller-supplied `reqwest::ClientBuilder`, keeping
    /// its proxy, TLS and pool configuration.
    ///
    /// The redirect policy is set to [`reqwest::redirect::Policy::none`] here
    /// for the reason [`KaleidoMakerClient::with_client_builder`] gives: the
    /// per-swap `swapAuth` this client carries on a chain re-quote is a custom
    /// header and follows any redirect.
    pub fn with_client_builder(
        options: KaleidoOrgClientOptions,
        http_client: reqwest::ClientBuilder,
    ) -> Result<Self, Error> {
        let KaleidoOrgClientOptions {
            maker_url,
            org_key,
            timeout,
        } = options;
        Self::validate_maker_url(&maker_url)?;
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let http_client = http_client.redirect(reqwest::redirect::Policy::none());
        let http_client = http_client.build()?;
        Ok(Self {
            inner: BoltzApiClientV2::with_client(maker_url, http_client, timeout)
                .with_org_key(org_key),
            org_key,
        })
    }

    fn validate_maker_url(maker_url: &str) -> Result<(), Error> {
        let url = reqwest::Url::parse(maker_url)?;
        let Some(host) = url.host_str() else {
            return Err(Error::Protocol(format!(
                "maker URL {maker_url} names no host — the organization key is \
                 bound to one origin and there is nothing here to bind it to"
            )));
        };
        match url.scheme() {
            "https" => Ok(()),
            "http" if is_loopback(host) => Ok(()),
            "http" => Err(Error::Protocol(format!(
                "refusing to attribute swaps to {maker_url} over plain HTTP — \
                 anything on the path could strip or replace the organization \
                 key. Use https, or the unattributed BoltzApiClientV2"
            ))),
            scheme => Err(Error::Protocol(format!(
                "maker URL {maker_url} is {scheme}, and the SDK speaks http(s)"
            ))),
        }
    }

    /// The configured organization key.
    pub fn org_key(&self) -> OrgKey {
        self.org_key
    }

    /// The generic client underneath, for code that takes one by reference.
    pub fn client(&self) -> &BoltzApiClientV2 {
        &self.inner
    }

    /// Take the generic client out. It keeps the key and the origin it is
    /// bound to.
    pub fn into_inner(self) -> BoltzApiClientV2 {
        self.inner
    }
}

impl Deref for KaleidoOrgClient {
    type Target = BoltzApiClientV2;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Whether a host is this machine.
///
/// Textual rather than resolved: a name that resolves to a loopback address
/// today can resolve elsewhere tomorrow, and the question being asked is whether
/// the plain-HTTP exemption applies, not where the packet ends up.
fn is_loopback(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.trim_start_matches('[').trim_end_matches(']').parse() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(std::net::IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    /// A key Platform could actually issue: a ULID-shaped id and a base64url
    /// secret, which is the case that makes the parse split where it does.
    const KEY: &str = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz";
    const SECRET: &str = "s3cr3t-Ab_Cd0123456789xyz";

    /// The secret runs to the end of the value, underscores and all.
    ///
    /// Splitting the whole key on `_` would cut a base64url secret in half and
    /// send a truncated credential — a `401` the caller would read as a revoked
    /// key. Only the three structural underscores are separators.
    #[test]
    fn a_key_parses_into_its_parts_and_the_secret_keeps_its_underscores() {
        let key = ApiKey::parse(KEY).unwrap();

        assert_eq!(key.environment(), ApiKeyEnvironment::Test);
        assert_eq!(key.key_id(), "01KZZYB138E7C3HZX7Q1YBGAQG");

        // The only place the secret is observable is the header it is sent in,
        // which is also the property that matters: the wire value must be the
        // key the partner pasted, byte for byte.
        let sent = key.bearer_header_value().unwrap();
        assert_eq!(sent.to_str().unwrap(), format!("Bearer {KEY}"));
        assert!(sent.to_str().unwrap().ends_with(SECRET));

        assert_eq!(
            ApiKey::parse("kld_live_abc-123_zzz").unwrap().environment(),
            ApiKeyEnvironment::Live,
        );
    }

    /// A header value carrying a credential must be marked sensitive: `hyper`
    /// keeps it out of its header dumps, and over HTTP/2 out of the HPACK
    /// dynamic table, where a connection-level observer or a later request on
    /// the same connection could otherwise index it.
    #[test]
    fn the_key_is_sent_as_a_sensitive_header_value() {
        assert!(ApiKey::parse(KEY)
            .unwrap()
            .bearer_header_value()
            .unwrap()
            .is_sensitive());
    }

    /// A value that cannot be a key must fail here, naming what is wrong with
    /// it.
    ///
    /// The alternative is a `401` from the maker, which is the same answer it
    /// gives for a revoked key and for a suspended organization — so a typo, an
    /// expired key and a truncated paste would all read as "your organization
    /// has been suspended".
    #[test]
    fn a_value_that_cannot_be_a_key_is_rejected_before_any_request() {
        for (value, expected) in [
            ("", "empty"),
            ("   ", "empty"),
            ("sk_test_abc_secret", "kld_"),
            ("kldtestabcsecret", "kld_"),
            ("kld_", "environment segment"),
            ("kld_staging_abc_secret", "not a key environment"),
            ("kld_test_abc", "no secret segment"),
            ("kld_test__secret", "key id is empty"),
            ("kld_test_abc_", "secret is empty"),
            ("kld_test_ab c_secret", "key id has a character"),
            ("kld_test_abc_sec ret", "cannot travel in a header"),
            ("kld_test_abc_sec\nret", "cannot travel in a header"),
            ("kld_test_abc_sécret", "cannot travel in a header"),
        ] {
            let err = ApiKey::parse(value).unwrap_err();
            assert!(
                matches!(&err, Error::Protocol(msg) if msg.contains(expected)),
                "parsing {value:?} should have complained about {expected:?}, got {err:?}",
            );
        }

        let oversized = format!("kld_test_abc_{}", "x".repeat(ApiKey::MAX_LEN));
        let err = ApiKey::parse(&oversized).unwrap_err();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains("bytes long")),
            "an oversized value should say so, got {err:?}",
        );
        // ...and must not quote the thing it just refused to accept as a key.
        assert!(!format!("{err:?}").contains("xxxx"), "{err:?}");
    }

    /// A key read from a file or a heredoc arrives with a trailing newline. No
    /// byte of a key is whitespace, so trimming cannot turn one valid key into
    /// a different valid key — and not trimming makes `KALEIDOSWAP_API_KEY` fail
    /// for a reason the caller cannot see in their terminal.
    #[test]
    fn surrounding_whitespace_does_not_make_a_key_unusable() {
        let from_a_file = format!("  {KEY}\n");
        let key = ApiKey::parse(&from_a_file).unwrap();
        assert_eq!(key.key_id(), "01KZZYB138E7C3HZX7Q1YBGAQG");
        assert_eq!(
            key.bearer_header_value().unwrap().to_str().unwrap(),
            format!("Bearer {KEY}"),
        );
    }

    /// Nothing that renders a key, a client, or the options used to build one
    /// may print the secret.
    ///
    /// The key is permanent until revoked, and `{:?}` on a client is the
    /// ordinary way to trace a misconfigured integration — so a derived `Debug`
    /// anywhere on this path would put the credential in whatever log that trace
    /// goes to, for as long as the log is kept.
    #[test]
    fn nothing_that_renders_a_key_prints_its_secret() {
        let key = ApiKey::parse(KEY).unwrap();
        let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
            maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
            api_key: key.clone(),
            timeout: None,
        })
        .unwrap();

        for rendered in [
            format!("{key:?}"),
            key.redacted(),
            format!("{client:?}"),
            format!("{:?}", client.client()),
            format!(
                "{:?}",
                KaleidoMakerClientOptions {
                    maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
                    api_key: key.clone(),
                    timeout: None,
                }
            ),
        ] {
            assert!(
                !rendered.contains(SECRET),
                "the key secret must not reach a log line, got:\n{rendered}",
            );
            assert!(
                rendered.contains("<redacted>"),
                "a redacted key should say so, got:\n{rendered}",
            );
            // The identifying half still prints, so the redaction did not cost
            // the value its usefulness in a log.
            assert!(
                rendered.contains("01KZZYB138E7C3HZX7Q1YBGAQG"),
                "the key id is public and worth seeing, got:\n{rendered}",
            );
        }
    }

    /// A bearer credential over plain HTTP is readable by anything on the path,
    /// and an organization key read once has to be revoked. Loopback is the
    /// regtest harness, where the "network" is a socket on this machine.
    #[test]
    fn a_maker_url_that_cannot_carry_the_key_is_refused() {
        let build = |maker_url: &str| {
            KaleidoMakerClient::new(KaleidoMakerClientOptions {
                maker_url: maker_url.to_string(),
                api_key: ApiKey::parse(KEY).unwrap(),
                timeout: None,
            })
        };

        for allowed in [
            "https://maker.signet.kaleidoswap.com/v2",
            "http://localhost:9001/v2",
            "http://127.0.0.1:9001/v2",
            "http://[::1]:9001/v2",
        ] {
            assert!(build(allowed).is_ok(), "{allowed} should be allowed");
        }

        let err = build("http://maker.signet.kaleidoswap.com/v2").unwrap_err();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains("plain")),
            "plain HTTP to a remote host must be refused, got {err:?}",
        );

        // A host that merely *looks* loopback is not: `localhost.example.com` is
        // somebody else's domain.
        assert!(build("http://localhost.example.com/v2").is_err());

        assert!(build("ftp://maker.signet.kaleidoswap.com/v2").is_err());
        assert!(build("not a url").is_err());

        // Userinfo in the URL is not a second way to authenticate: `reqwest`
        // turns it into an `Authorization: Basic …` of its own, and
        // `RequestBuilder::header` appends rather than replaces — so the request
        // would carry two `Authorization` headers and the maker, reading the
        // first, would never see the key. In its optional mode that is a swap
        // silently recorded as anonymous.
        for with_userinfo in [
            "https://ci:hunter2@maker.signet.kaleidoswap.com/v2",
            "https://ci@maker.signet.kaleidoswap.com/v2",
        ] {
            let err = build(with_userinfo).unwrap_err();
            assert!(
                matches!(&err, Error::Protocol(msg) if msg.contains("username or password")),
                "{with_userinfo} must be refused, got {err:?}",
            );
            assert!(!format!("{err:?}").contains("hunter2"), "{err:?}");
        }

        // ...and the generic client is unaffected: it carries no credential, so
        // a plain-HTTP maker is the caller's business.
        assert!(
            BoltzApiClientV2::new("http://maker.example.com/v2".to_string(), None)
                .api_key()
                .is_none()
        );
    }

    /// `with_client_builder` must apply exactly the same URL policy as `new`.
    ///
    /// It is the constructor a caller reaches for to get a proxy or a custom TLS
    /// setup, and it would be a quiet way around every check above if it took
    /// the URL on trust.
    #[test]
    fn a_caller_supplied_client_is_held_to_the_same_url_policy() {
        let build = |maker_url: &str| {
            KaleidoMakerClient::with_client_builder(
                KaleidoMakerClientOptions {
                    maker_url: maker_url.to_string(),
                    api_key: ApiKey::parse(KEY).unwrap(),
                    timeout: None,
                },
                reqwest::Client::builder(),
            )
        };

        let client = build("https://maker.signet.kaleidoswap.com/v2").unwrap();
        assert_eq!(client.api_key_id(), "01KZZYB138E7C3HZX7Q1YBGAQG");

        assert!(build("http://maker.signet.kaleidoswap.com/v2").is_err());
        assert!(build("https://ci:hunter2@maker.signet.kaleidoswap.com/v2").is_err());
        assert!(build("ftp://maker.signet.kaleidoswap.com/v2").is_err());
    }

    /// The x coordinate of the secp256k1 generator: a key that is certainly on
    /// the curve.
    const ORG_KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    /// A key parses from either case and prints back as the lowercase the
    /// maker, Platform and the panel all store, so one key never reads as two
    /// organizations.
    #[test]
    fn an_org_key_parses_from_either_case_and_prints_lowercase() {
        let key = OrgKey::parse(ORG_KEY).unwrap();
        assert_eq!(key.to_string(), ORG_KEY);
        assert_eq!(OrgKey::parse(&ORG_KEY.to_uppercase()).unwrap(), key);
        assert_eq!(OrgKey::parse(&format!("  {ORG_KEY}\n")).unwrap(), key);
        assert_eq!(key.header_value().to_str().unwrap(), ORG_KEY);
        assert_eq!(
            OrgKey::from(key.x_only_public_key()),
            key,
            "the x-only key round-trips",
        );
    }

    /// The key is public, and everything that renders it shows it: unlike an
    /// [`ApiKey`], the value is what a partner matches against the panel.
    #[test]
    fn an_org_key_renders_and_serializes_as_its_hex() {
        let key = OrgKey::parse(ORG_KEY).unwrap();
        assert_eq!(format!("{key:?}"), format!("OrgKey(\"{ORG_KEY}\")"));

        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, format!("\"{ORG_KEY}\""));
        assert_eq!(serde_json::from_str::<OrgKey>(&json).unwrap(), key);
        assert!(serde_json::from_str::<OrgKey>("\"abc\"").is_err());

        // Header values for a public label are not marked sensitive: there is
        // nothing to keep out of a header dump.
        assert!(!key.header_value().is_sensitive());
    }

    /// A value that cannot be a key fails here, naming what is wrong, rather
    /// than reaching the maker as `400 org_key_malformed`.
    ///
    /// The two off-curve values are BIP-340's own test vectors: an x with no
    /// point on the curve, and an x past the field size.
    #[test]
    fn a_value_that_cannot_be_an_org_key_is_rejected_before_any_request() {
        for (value, expected) in [
            (String::new(), "0 characters"),
            (ORG_KEY[..63].to_string(), "63 characters"),
            (format!("{ORG_KEY}0"), "65 characters"),
            (ORG_KEY.replace('7', "g"), "not hex"),
            (
                "eefdea4cdb677750a420fee807eacf21eb9898ae79b9768766e4faa04a2d4a34".to_string(),
                "point on the secp256k1 curve",
            ),
            (
                "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc30".to_string(),
                "point on the secp256k1 curve",
            ),
            // A compressed key is the wrong shape, not a key to truncate.
            (format!("02{ORG_KEY}"), "66 characters"),
            // A kld_ key in the wrong field.
            (
                "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t".to_string(),
                "characters long",
            ),
        ] {
            let err = OrgKey::parse(&value).unwrap_err();
            assert!(
                matches!(&err, Error::Protocol(msg) if msg.contains(expected)),
                "parsing {value:?} should have complained about {expected:?}, got {err:?}",
            );
        }
    }

    /// The org-key client takes the same URL policy as the API-key one, for a
    /// different reason: the key is public, but over plain HTTP anything on the
    /// path can strip or swap it.
    #[test]
    fn an_org_key_client_refuses_a_url_that_cannot_carry_attribution() {
        let build = |maker_url: &str| {
            KaleidoOrgClient::new(KaleidoOrgClientOptions {
                maker_url: maker_url.to_string(),
                org_key: OrgKey::parse(ORG_KEY).unwrap(),
                timeout: None,
            })
        };

        for allowed in [
            "https://maker.signet.kaleidoswap.com/v2",
            "http://localhost:9001/v2",
            "http://127.0.0.1:9001/v2",
            "http://[::1]:9001/v2",
        ] {
            assert!(build(allowed).is_ok(), "{allowed} should be allowed");
        }
        let err = build("http://maker.signet.kaleidoswap.com/v2").unwrap_err();
        assert!(
            matches!(&err, Error::Protocol(msg) if msg.contains("plain HTTP")),
            "{err:?}",
        );
        assert!(build("http://localhost.example.com/v2").is_err());
        assert!(build("ftp://maker.signet.kaleidoswap.com/v2").is_err());
        assert!(build("not a url").is_err());

        let with_builder = KaleidoOrgClient::with_client_builder(
            KaleidoOrgClientOptions {
                maker_url: "http://maker.signet.kaleidoswap.com/v2".to_string(),
                org_key: OrgKey::parse(ORG_KEY).unwrap(),
                timeout: None,
            },
            reqwest::Client::builder(),
        );
        assert!(
            with_builder.is_err(),
            "the builder path has the same policy"
        );
    }

    /// An org-key client carries its key and no API key, and keeps the key when
    /// unwrapped, so handing the inner client on does not drop attribution.
    #[test]
    fn an_org_key_client_carries_only_its_org_key() {
        let key = OrgKey::parse(ORG_KEY).unwrap();
        let client = KaleidoOrgClient::new(KaleidoOrgClientOptions {
            maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
            org_key: key,
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();

        assert_eq!(client.org_key(), key);
        assert_eq!(client.client().org_key(), Some(key));
        assert!(client.api_key().is_none(), "never both on one client");
        assert_eq!(
            client.get_ws_url(),
            "wss://maker.signet.kaleidoswap.com/v2/ws"
        );
        assert_eq!(client.into_inner().org_key(), Some(key));

        // ...and the API-key client carries no org key.
        let api_client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
            maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
            api_key: ApiKey::parse(KEY).unwrap(),
            timeout: None,
        })
        .unwrap();
        assert!(api_client.org_key().is_none());
    }

    /// The Kaleido client must be usable everywhere the generic one is, or the
    /// key becomes a reason to keep two code paths.
    #[test]
    fn the_kaleido_client_is_the_generic_client_plus_a_key() {
        let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
            maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
            api_key: ApiKey::parse(KEY).unwrap(),
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();

        assert_eq!(client.api_key_environment(), ApiKeyEnvironment::Test);
        assert_eq!(client.api_key_id(), "01KZZYB138E7C3HZX7Q1YBGAQG");
        // Deref reaches the maker routes.
        assert_eq!(
            client.get_ws_url(),
            "wss://maker.signet.kaleidoswap.com/v2/ws",
        );

        // And the key survives being unwrapped, so handing the inner client to
        // code that takes a `BoltzApiClientV2` does not silently drop attribution.
        let inner = client.into_inner();
        assert_eq!(
            inner.api_key().map(ApiKey::key_id),
            Some("01KZZYB138E7C3HZX7Q1YBGAQG"),
        );
    }
}
