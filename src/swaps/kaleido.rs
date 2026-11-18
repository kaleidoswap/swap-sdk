//! KaleidoSwap partner attribution: the partner's `kld_` key and the client
//! that carries it.
//!
//! A partner organization gets a key from the KaleidoSwap Platform panel and
//! configures it here. The maker introspects the key, resolves it to an
//! organization, and stores that organization with the swap, so the partner can
//! later see statistics for the swaps they originated.
//!
//! # The key is a public partner identifier
//!
//! A `kld_<env>_<key_id>_<secret>` key identifies a partner; it is not a
//! secret. Ship it inside the app as a build constant: a browser page, a Web,
//! shared or service worker, a browser extension, a mobile or desktop app, or a
//! server. Anyone who unpacks the app can read it, and that is fine, because a
//! key never buys its holder anything:
//!
//! - it attributes the swap to the partner;
//! - it carries the partner's fee, set on the key in the panel, which the maker
//!   adds on top of its own price and reports as a separate `partner` fee line
//!   (`partner_percentage` on a rate card's fees, `partner` on a quote's);
//! - it never gives a lower price, higher limits or a rate-limit budget of its
//!   own, so copying it gains nothing.
//!
//! The partner fee comes only from the key: no request field sets it, and the
//! SDK sends no per-request fee of its own.
//!
//! Send the key on every price read and every create, not only on the create.
//! A keyed caller's rate card (`/pairs`) already includes the partner fee in
//! `fees.percentage` and carries its own `pairHash`, so amounts computed from
//! an anonymous card and created with a key fail the create with
//! `pair_hash_mismatch`. [`KaleidoMakerClient`] does this for you: the key goes
//! out on every request to the configured maker, and to no other host.
//!
//! A key the maker does not recognise (unknown, revoked, or for the other
//! environment) is not an error: the maker answers with the public price and
//! leaves the swap unattributed, and says so in [`ATTRIBUTION_HEADER`]. The
//! client keeps that answer as data: see
//! [`BoltzApiClientV2::last_attribution`].
//!
//! The key authorizes nothing. It answers *"which partner brought this
//! swap?"* — no claim, no refund, no fund movement, no panel access and no
//! administrative operation. The per-swap credential the maker issues as
//! `swapAuth` is what answers *"is this caller allowed to decide the outcome
//! of this specific swap?"*, and the two stay separate: see
//! [`SWAP_AUTH_HEADER`].
//!
//! [`SWAP_AUTH_HEADER`]: crate::boltz::SWAP_AUTH_HEADER
//! [`BoltzApiClientV2`]: crate::boltz::BoltzApiClientV2
//! [`BoltzApiClientV2::last_attribution`]: crate::boltz::BoltzApiClientV2::last_attribution
//!
//! ```no_run
//! use kaleidorg_swap_sdk::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};
//!
//! /// The partner's key, built into the app.
//! const KALEIDOSWAP_PARTNER_KEY: &str = "kld_live_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t";
//!
//! # async fn run() -> Result<(), kaleidorg_swap_sdk::error::Error> {
//! let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
//!     maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
//!     api_key: KALEIDOSWAP_PARTNER_KEY.parse::<ApiKey>()?,
//!     timeout: None,
//! })?;
//!
//! // Every maker route is reachable through the deref to the generic client.
//! let pairs = client.get_submarine_pairs().await?;
//! # let _ = pairs;
//! // Whose price the maker answered with: `api_key`, or `none` for a key it
//! // did not recognise.
//! let _attribution = client.last_attribution();
//! # Ok(())
//! # }
//! ```
//!
//! # A secret key kind, later
//!
//! Every key Platform issues today is public ([`ApiKeyKind::Public`]). A
//! secret, server-only kind may follow; the SDK keeps its refusal to run such a
//! key in a browser or an app ready for it, and applies it to that kind only.

use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use std::time::Duration;

use reqwest::header::HeaderValue;
use zeroize::Zeroizing;

use crate::error::Error;
use crate::swaps::boltz::BoltzApiClientV2;

/// The header the partner key travels in, as a `Bearer` credential.
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

/// The prefix every KaleidoSwap partner key carries.
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

/// What a `kld_` key may be shipped in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApiKeyKind {
    /// A public partner identifier: it attributes swaps and carries the
    /// partner's fee, and never a lower price or higher limits. Safe in a
    /// browser, an extension, a mobile or desktop app and a server alike.
    /// Every key Platform issues today is public.
    Public,
    /// A server-only key. Platform issues none yet; the SDK refuses to build
    /// a client for one in a browser or an app ([`Self::requires_server`]).
    Secret,
}

impl ApiKeyKind {
    /// Whether a key of this kind must stay on a server. `false` for
    /// [`Self::Public`]: those keys are meant to ship inside apps.
    pub fn requires_server(&self) -> bool {
        matches!(self, Self::Secret)
    }
}

/// A parsed KaleidoSwap partner key: `kld_<env>_<key_id>_<secret>`.
///
/// Parsing is structural, not a validity check — only the Platform knows whether
/// a well-formed key is active, and only the maker's introspection call can say
/// so. What parsing buys is that a value which cannot possibly be a key fails
/// here, naming itself, instead of arriving as a `401` that reads exactly like a
/// revoked key or a suspended organization.
///
/// Every key Platform issues today is a public partner identifier
/// ([`ApiKeyKind::Public`]), safe to build into an app; see the
/// [module docs][self]. The last segment is still kept out of logs: there is
/// no accessor for it, no `Display`, no `Serialize`, and [`Debug`] renders
/// [`Self::redacted`]. Nothing is gained by printing it, and a future secret
/// key kind shares the format.
///
/// It is held in [`Zeroizing`], which wipes the stored copy and the per-request
/// buffer on drop. That bounds the exposure rather than ending it: the
/// `HeaderValue` each request carries, the buffer hyper encodes it into, and the
/// string the caller handed to [`Self::parse`] are all outside this type and
/// none of them are wiped.
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
            "not a KaleidoSwap partner key: {why}. Keys look like \
             `kld_test_<key_id>_<secret>` and are issued in the partner panel"
        ))
    }

    /// The environment the key is scoped to.
    pub fn environment(&self) -> ApiKeyEnvironment {
        self.environment
    }

    /// What kind of key this is: [`ApiKeyKind::Public`] for every key
    /// Platform issues today. The format carries no kind, so a key parsed here
    /// is public until Platform issues a secret kind the SDK can tell apart.
    pub fn kind(&self) -> ApiKeyKind {
        ApiKeyKind::Public
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
                "partner key {} cannot be sent as an {API_KEY_HEADER} value",
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

/// The header the maker answers swap creations and price reads with: `none`
/// or `api_key`.
///
/// On a create it names the attribution the swap was actually stored with; on
/// a price read (a rate card or a quote) it says whose price the maker
/// answered with: `api_key` for the partner key's price, `none` for the public
/// price. A key the maker does not recognise (unknown, revoked, or for the
/// other environment) gets `none` and the public price rather than an error,
/// so an app with a rotated key keeps swapping.
///
/// Every client keeps the last value it saw, as a [`MakerAttribution`]:
/// [`BoltzApiClientV2::last_attribution`]. A client with a key also logs a
/// warning when a create comes back with anything other than its own kind of
/// attribution. In a browser the header is readable only where the maker's
/// edge exposes it, and its absence is not reported.
///
/// [`BoltzApiClientV2::last_attribution`]: crate::boltz::BoltzApiClientV2::last_attribution
pub const ATTRIBUTION_HEADER: &str = "X-Kaleido-Attribution";

/// What the maker said in [`ATTRIBUTION_HEADER`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MakerAttribution {
    /// `none`: the public price, or a swap stored without a partner. For a
    /// client with a key this means the maker did not recognise it: unknown,
    /// revoked, or for the other environment.
    Unattributed,
    /// `api_key`: the partner key's price, or a swap stored for that partner.
    ApiKey,
    /// A value this SDK does not know yet, kept verbatim.
    Other(String),
}

impl MakerAttribution {
    /// Read a header value. Never fails: an unknown value is kept as
    /// [`Self::Other`].
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "none" => Self::Unattributed,
            "api_key" => Self::ApiKey,
            other => Self::Other(other.to_string()),
        }
    }

    /// The header value, as the maker sent it.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unattributed => "none",
            Self::ApiKey => "api_key",
            Self::Other(other) => other,
        }
    }
}

impl Display for MakerAttribution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How to build a [`KaleidoMakerClient`].
#[derive(Debug)]
pub struct KaleidoMakerClientOptions {
    /// The maker's `/v2` base URL — and the only origin the key is ever sent to.
    pub maker_url: String,
    /// The partner's `kld_` key from the partner panel. Public: build it into
    /// the app.
    pub api_key: ApiKey,
    /// Per-request timeout. `None` leaves it to the HTTP stack.
    pub timeout: Option<Duration>,
}

/// A [`BoltzApiClientV2`] that sends a partner's `kld_` key to the maker, so
/// the swaps it creates are attributed to the partner and priced with the
/// partner's fee.
///
/// The key goes out on every request to the configured maker: the rate cards
/// and quotes as well as the creates, so the amounts and the `pairHash` a swap
/// is created with come from the partner's own card. It is bound to one origin
/// at construction and refuses to travel anywhere else, so a
/// [`BoltzApiClientV2`] built for Boltz, Esplora or a second maker cannot end up
/// carrying it. Works in a browser, a worker, an extension, an app and a
/// server alike: the key is public. See the [module docs][self].
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
    /// In the browser `fetch` owns redirect handling and wasm-bindgen can set
    /// no policy on it; there the SDK issues every maker `fetch` with
    /// `redirect: "error"` itself, and
    /// [`BoltzApiClientV2::reject_credential_leaking_redirect`] reports a hop
    /// after the fact.
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
    /// `https`, or `http` to a loopback host. The key is public, so this is
    /// not about secrecy: over plain HTTP anything on the path can strip or
    /// replace it, and the swap is priced and attributed for somebody else or
    /// for nobody with no sign of it on either end. Loopback is exempt because
    /// that is the regtest harness, where the "network" is a socket on the same
    /// machine.
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
                "maker URL {maker_url} names no host — the partner key is \
                 bound to one origin and there is nothing here to bind it to"
            )));
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Error::Protocol(format!(
                "maker URL for {host} carries a username or password — those become \
                 an Authorization header of their own, which would displace the \
                 partner key and leave the swap unattributed. Put the \
                 credentials in your own reqwest client instead"
            )));
        }
        match url.scheme() {
            "https" => Ok(()),
            "http" if is_loopback(host) => Ok(()),
            "http" => Err(Error::Protocol(format!(
                "refusing to send a partner key to {maker_url} over plain HTTP — \
                 anything on the path could strip or replace it. Use https, or the \
                 unattributed BoltzApiClientV2"
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

    /// The configured key's kind: [`ApiKeyKind::Public`] for every key
    /// Platform issues today.
    pub fn api_key_kind(&self) -> ApiKeyKind {
        self.api_key_ref().kind()
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
        // Every key Platform issues today is a public partner identifier, so
        // nothing about it keeps the client out of a browser or an app.
        assert_eq!(client.api_key_kind(), ApiKeyKind::Public);
        assert!(!client.api_key_kind().requires_server());
        assert!(ApiKeyKind::Secret.requires_server());
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
