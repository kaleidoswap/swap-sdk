# kaleidorg-swap-sdk

**KaleidoSwap swap SDK** — client-side atomic swaps (Boltz protocol) across
Bitcoin, Lightning, and Liquid, for Rust, Python, the browser, and React
Native.

The published surface is **swaps-only**: quote/create/watch a swap against the
KaleidoSwap maker, derive per-swap keys and preimages, and build the claim /
refund transactions client-side. Any Boltz-`/v2`-compatible endpoint works via
an explicit base URL. Endpoint support varies by maker and binding; see the
[maker/upstream compatibility matrix](docs/compatibility.md) for submarine
co-signing, public raw broadcast and BOLT12 routes.

### Networks

| `Network` | Default maker | Bitcoin chain access |
| --- | --- | --- |
| `Signet` | KaleidoSwap — `maker.signet.kaleidoswap.com/v2` | KaleidoSwap — `esplora.signet.kaleidoswap.com` (Mutinynet) |
| `Regtest` | local harness — `localhost:9001/v2` | `BitcoinRegtest` → local |
| `Testnet` | **errors** — no KaleidoSwap testnet3 maker; use `Signet` | Blockstream — `blockstream.info/testnet/api` |
| `Mainnet` | **errors** — no mainnet maker yet | Blockstream — `blockstream.info/api` |

`BoltzApiClientV2::default` only ever returns a KaleidoSwap maker. On a network
we run no maker on it errors instead of falling back to a third party, so a
default can never put your swap in front of a counterparty you did not choose.
Other makers stay reachable by name — pass an explicit base URL to
`BoltzApiClientV2::new` (`BOLTZ_TESTNET_URL_V2` / `BOLTZ_MAINNET_URL_V2` for
Boltz). **Signet is our testing network**, and on it both defaults — maker and
chain access — are KaleidoSwap infrastructure. Mainnet and testnet3 chain
defaults remain public explorers, which is only reachable once you have named a
third-party maker explicitly anyway; pass your own URL to
`EsploraBitcoinClient::new` to avoid them. Liquid chain access has no KaleidoSwap
explorer yet and defaults to Blockstream.

The KaleidoSwap maker settles on **Mutinynet**, a custom signet. Signet and
testnet3 share an address encoding, so mixing a signet maker with testnet3
chain access fails *silently* rather than erroring — always keep the `Network`
and the chain client on the same row. Mutinynet has no public Electrum server,
so `ElectrumBitcoinClient::default` errors for signet; use Esplora, or pass your
own Electrum URL to `ElectrumBitcoinClient::new`.

The crate is a fork of [boltz-rust](https://github.com/SatoshiPortal/boltz-rust):
the battle-tested swap engine (taproot swap scripts, MuSig2 cooperative signing,
claim/refund transaction construction, BIP85 key derivation) is kept intact, and
the KaleidoSwap layers are built on top of it.

## Arkade Intents corridor (`arkade:BTC <-> lightning:BTC`)

Bitcoin on Arkade (`ARKD` in the maker's catalogue) is not a Boltz-shaped
route. The maker serves it as an RFQ over `POST /v1/swap` and
`GET /v1/rfq/{id}`, beside its `/v2` surface, and this SDK speaks that wire from
`kaleidorg_swap_sdk::corridor` on the same client:

```rust
use kaleidorg_swap_sdk::corridor::{new_rfq_id, LightningSendRequest, RfqAnswer};

let answer = client
    .quote_lightning_send(&LightningSendRequest {
        rfq_id: new_rfq_id(),
        invoice,                 // the BOLT11 you want paid
        refund_address,          // your own Ark address
        client_refund_pubkey,    // your x-only key, hex
    })
    .await?;
match answer {
    RfqAnswer::Quote(quote) => {
        quote.assert_fundable(now)?;   // 90-minute headroom before the refund deadline
        // Derive the covenant from the quote's binding fields, compare it against
        // quote.profile.lockup_address, then fund quote.from_amount sats there.
    }
    RfqAnswer::Refusal(refusal) => eprintln!("refused: {:?}", refusal.reason),
}
```

A refusal is the maker's answer, not an error — it comes back as a value. The
same surface is `SwapClient.quote_lightning_send(...)` in Python and
`new IntentsCorridor(client).quoteLightningSend(...)` in TypeScript.

Discover the maker's advertised markets with `client.corridor_card().await?`
in Rust, `await client.corridor_card()` in Python, or
`await new IntentsCorridor(client).card()` in TypeScript (also exposed as
`client.corridorCard()`). The method requests `GET /v1/card` beside the client's
`/v2` base, preserving any deployment path prefix and client authentication.
It returns the maker's card (signature not verified), with amount limits kept
as decimal strings,
or `None`/`null` for HTTP 404 (unavailable or withdrawn). Other HTTP, transport
and malformed-response failures remain errors. Fetching the card does not
verify its signature or establish trust in its advertised discovery key.
Only schema v0 is accepted. Use `corridor_card_raw()` / `cardRaw()` to preserve
unknown fields for verification; see the TypeScript README for the signing
preimage and lossless-processing requirements.

What this half does **not** do is touch Arkade: funding a send lockup or
claiming a receive lockup needs an Ark wallet, which is the
`@kaleidorg/swap-sdk/arkade` venue's job (TypeScript, on `@arkade-os/sdk`).
Build that venue's transport from the same maker URL with
`kaleidoswapHttpTransport("https://maker.signet.kaleidoswap.com/v2")`.

## Partner attribution

A partner organization can have the swaps it originates attributed to it, so the
volume and fees it drives show up in its own statistics. Attribution is opt-in,
and there are two ways to do it. Without either, every client here behaves
exactly as before and creates unattributed swaps.

- **Client-side software** (a web page, an extension, a mobile or desktop app)
  uses the organization's public **organization key**. There is nothing secret
  in it to ship.
- **Servers** may instead use a secret **organization API key**, which also
  unlocks what a public label cannot: a rate-limit budget of the organization's
  own and access to a partners-only maker.

### Organization keys (client-side)

An organization key is the 64-hex x-only secp256k1 public key the organization
registered in the partner panel, proving with a signature that it holds the
private half. That private half is used only to claim a revshare payout, on the
organization's own machine; no KaleidoSwap software, this SDK included, ever
asks for it.

The key is a **label, never a credential**. Anyone can copy it, so it buys the
caller nothing: no rate-limit budget of its own, no discount, no access in the
maker's partners-only mode. That is what makes it safe in a bundle.

```rust
use kaleidorg_swap_sdk::kaleido::{KaleidoOrgClient, KaleidoOrgClientOptions, OrgKey};

let client = KaleidoOrgClient::new(KaleidoOrgClientOptions {
    maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
    org_key: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse::<OrgKey>()?,
    timeout: None,
})?;
```

```python
client = kaleidorg_swap_sdk.SwapClient.kaleido_org(
    "https://maker.signet.kaleidoswap.com/v2", ORG_KEY, None
)
```

```ts
import { createKaleidoOrgClient } from "@kaleidorg/swap-sdk";

const client = createKaleidoOrgClient({
  makerUrl: "https://maker.signet.kaleidoswap.com/v2",
  orgKey: import.meta.env.VITE_KALEIDOSWAP_ORG_KEY,
});
```

The key goes out in `X-Kaleido-Org-Key` on swap creation only — `POST
/v2/swap/{submarine,reverse,chain}`, `POST /v2/swap/pay` and the corridor's
`POST /v1/swap` — and only to the configured maker URL, which must be `https`
unless it is a loopback address. A value that is not a point on the curve is
rejected locally rather than reaching the maker as `400 org_key_malformed`. The
maker answers every creation with `X-Kaleido-Attribution: none | api_key |
org_key`; when an org-keyed creation comes back as anything but `org_key` (a
maker with organization keys switched off, say), the swap still goes ahead and
the SDK logs a warning. A page on another origin also needs the maker's edge to
allow the header in CORS. The TypeScript `PayThroughClient` and
`kaleidoswapHttpTransport` take an `orgKey` option too.

### Organization API keys (servers)

The secret key comes from the same panel: a `kld_test_…` key for signet and
staging, `kld_live_…` for mainnet and production.

The key answers one question — *which partner organization created this swap?* —
and nothing else. It authorizes no claim, no refund, no fund movement and no
panel access. The per-swap `swapAuth` credential the maker returns on create is
what authorizes the outcome of a specific swap, and the two stay separate.

```rust
use kaleidorg_swap_sdk::kaleido::{ApiKey, KaleidoMakerClient, KaleidoMakerClientOptions};

let client = KaleidoMakerClient::new(KaleidoMakerClientOptions {
    maker_url: "https://maker.signet.kaleidoswap.com/v2".to_string(),
    api_key: std::env::var("KALEIDOSWAP_API_KEY").unwrap().parse::<ApiKey>()?,
    timeout: None,
})?;
```

```python
client = kaleidorg_swap_sdk.SwapClient.kaleido_maker(
    "https://maker.signet.kaleidoswap.com/v2", os.environ["KALEIDOSWAP_API_KEY"], None
)
```

```ts
import { createKaleidoMakerClient } from "@kaleidorg/swap-sdk";

const client = createKaleidoMakerClient({
  makerUrl: "https://maker.signet.kaleidoswap.com/v2",
  apiKey: process.env.KALEIDOSWAP_API_KEY!,
});
```

Every maker route is available on the result — it is the ordinary client plus a
credential, not a second API.

**Credential handling.** The key travels as `Authorization: Bearer …`, marked
sensitive so it stays out of header dumps and out of the HTTP/2 HPACK dynamic
table. It is bound to the maker URL it was configured with and is never attached
to a request addressed anywhere else — not Esplora, not a second maker. That URL
must be `https` unless it is a loopback address, since a bearer credential over
plain HTTP is readable by anything on the path, and it may not carry userinfo —
`https://user:pw@host/v2` becomes an `Authorization: Basic …` of its own that
would displace the key and leave the swap unattributed. Redirects are declined
outright on both native constructors — `with_client_builder` takes a
`reqwest::ClientBuilder` rather than a built client precisely so the SDK can set
the policy while keeping your proxy and TLS configuration; only in the browser,
where `fetch` owns redirects, is a hop off the maker reported after the fact
instead. Nothing renders the secret: the
Rust `Debug` prints `kld_test_<key_id>_<redacted>`, Python and JS expose the key
id and environment and no accessor for the secret at all. A value that cannot be
a key is rejected locally rather than reaching the maker as a `401`, which is the
same answer a revoked key gets.

**Server and native integrations only.** A key in a browser bundle is visible to
every visitor, who can then attribute their own swaps to — or exhaust the limits
of — an organization that is not theirs. Keep it in server-side configuration;
for attribution from client-side code, use an organization key instead. A
client carries one or the other, never both: the maker refuses a request with
both as `400 attribution_conflict`.

## Repository structure

| Path | What it is |
|---|---|
| `src/` | The swap engine (Boltz protocol): scripts, MuSig2, tx construction, key/preimage derivation, Esplora/Electrum chain access |
| `bindings/` | [UniFFI](https://mozilla.github.io/uniffi-rs/) bindings — Python today, and the crate every native target is generated from |
| `bindings-wasm/` | wasm-bindgen bindings for the browser — same swap surface, 64-bit integers cross as `BigInt` |
| `typescript-sdk/` | TypeScript SDK (`@kaleidorg/swap-sdk`) wrapping the wasm package with hand-written types |
| `packages/react-native/` | React Native SDK (`@kaleidorg/swap-sdk-react-native`) — a turbo module over the UniFFI crate, because Hermes has no `WebAssembly` |
| `macros/` | Proc-macros (wasm-compatible `async_trait`, cross-target `test_all`) |

## Installation and supported runtimes

| Surface | Install | Supported runtime |
|---|---|---|
| Rust | `kaleidorg-swap-sdk = { git = "https://github.com/kaleidoswap/swap-sdk", tag = "v0.12.0" }` | Rust 1.88+, native and `wasm32-unknown-unknown` |
| Python | `pip install kaleidorg_swap_sdk` | Python 3.10+; wheels for Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64, sdist elsewhere |
| TypeScript | `npm install @kaleidorg/swap-sdk` | Browsers and Node 22+; `await init()` takes no argument in either |
| React Native | `npm install @kaleidorg/swap-sdk-react-native` | iOS and Android, new architecture; a bare or prebuilt app, not Expo Go |

Both registries are live and carry every release through `0.12.0`:
`kaleidorg_swap_sdk` on PyPI (five platform wheels plus an sdist) and
`@kaleidorg/swap-sdk` on npm. npm has no `0.1.0` — that version reached PyPI
only; see the `0.1.1` changelog entry. The Rust crate is not published to
crates.io — depend on it by tag.

The distribution rename to `kaleidorg_swap_sdk` is what cleared the public PyPI
collision that blocked the previous name (`kaleidoswap-sdk`, whose normalized
project already holds `0.1.0`-`0.5.6`). Publishing per tag is still gated on
`PYPI_PUBLISH_ENABLED` and `NPM_PUBLISH_ENABLED`, so read both before you tag.
See [the release guide](docs/releasing.md#release-architecture) for how the
publisher flags work and [artifact
sources](docs/releasing.md#python-registry).

## Generated sources

```bash
make generate-python-bindings  # platform-independent UniFFI Python glue fallback
make check-generated           # regenerate from pinned inputs and reject drift
make generate-swap-types       # serde-derived WASM/TypeScript swap DTOs
make check-swap-types           # reject DTO declaration drift
```

The generated fallback is committed. Pull-request CI regenerates it solely from
repository-pinned tool inputs and rejects any drift.

The six swap create/status/restore methods use public TypeScript DTOs generated
from the Rust types with pinned `ts-rs`. Their declarations are embedded in the
WASM package, so Node, browser and raw WASM consumers share the same signatures.
See [the TypeScript guide](typescript-sdk/README.md#swap-dto-generation-and-maker-schema-checks)
for maker schema compatibility checks and snapshot updates.

## Rebuild scripts after swap recovery

`SwapScript.fromRestored(entry, ourPubkeyHex, network, side?)` accepts an entry
returned by `SwapClient.swapRestore()` or `SwapMasterKey.restore()` directly.
Use the swap key at the selected details' `keyIndex`: refund details for a
submarine swap, claim details for a reverse swap. For chain swaps, pass
`"lockup"` or `"claim"` explicitly and derive the corresponding key.

```ts
const [entry] = await master.restore(client);
const refundKey = master.deriveSwapKey(BigInt(entry.refundDetails.keyIndex));
const submarine = SwapScript.fromRestored(entry, refundKey.publicKey, "signet");

// For a chain swap, select each side and its own key separately.
const claimKey = master.deriveSwapKey(BigInt(chainEntry.claimDetails.keyIndex));
const claim = SwapScript.fromRestored(
  chainEntry,
  claimKey.publicKey,
  "signet",
  "claim",
);
```

This reconstructs and validates the canonical script tree, keys, timeout,
network and lockup address without inventing create-response fields. It
preserves the expected on-chain amount and Liquid blinding key. It performs
no network requests and neither claims nor refunds automatically. Restore
claim details must include `amount`; refund details may omit it, because refunds
recover the positive amount actually locked. Refund hashlock validation prefers
the maker's `preimageHash` and otherwise supports BOLT11 or BOLT12 invoices.

BTC and L-BTC sides are supported for all three swap kinds, including explicit
and confidential Liquid addresses. The current restore wire contract omits
RGB allocation data and Liquid token asset/fee-asset context, so USDT-RGB,
L-USDT and Arkade sides are rejected explicitly. Persist their original
validated create responses and wallet context until the maker supplies that
recovery metadata. Restore also does not recover the maker's `swapAuth`
credential; use the separate signed credential recovery flow before a
cooperative claim or refund that requires it.

Native bindings expose the same core constructor as
`SwapScript.from_restored(network, restored_swap, our_pubkey, side)`.

## Bindings

- **Python (UniFFI):** see [`bindings/`](bindings/README.md). Build with
  `cd bindings && make build-debug`; examples in `bindings/python/examples/`.
- **Browser/TypeScript (wasm):** `make wasm-pack-build` builds the wasm package
  and vendors it into `typescript-sdk/`. See `typescript-sdk/src/index.ts` for
  the typed surface (`SwapClient`, `SwapScript`, `SwapMasterKey`,
  `SwapWsApi`).
- **React Native (UniFFI):** see
  [`packages/react-native/`](packages/react-native/README.md). `make rn-build`
  cross-compiles the bindings crate for iOS and Android and regenerates the
  turbo module. The Arkade corridor is not part of the native module — it stays
  TypeScript and is re-exported from `@kaleidorg/swap-sdk/arkade`.

  Published with every release beside the browser package: the release
  pipeline seals both npm tarballs in one bundle and attaches the compiled
  iOS and Android archives to the GitHub release, where the package's
  `postinstall` fetches them and accepts each only at the SHA-256 recorded in
  the manifest that shipped inside the tarball. The pipeline ends by
  installing the published version into a clean project with scripts enabled,
  so a release is not green until that install has produced the libraries.

`make check-binding-parity` fails when the wasm binding exposes a capability
the UniFFI binding does not, so the native surface cannot fall behind the
browser one unnoticed.

Pull-request packaging CI builds and clean-installs Python wheels for Linux
x86_64/aarch64, macOS x86_64/arm64, and Windows x86_64. It also reconstructs
the native package from the Python sdist and installs the packed npm tarball in
an isolated Node consumer.

## Releases

[`VERSIONING.md`](VERSIONING.md) states what a consumer can rely on between
releases — what `0.x` means here, how a breaking change is announced, and what
`1.0` is gated on. This section is how a release is cut.

All public SDK surfaces share one stable `X.Y.Z` version. A release candidate
starts only when a strict `vX.Y.Z` tag points to a commit reachable from
`trunk`, and the tag must match the Rust, Python, and TypeScript manifests and
lockfiles. `make validate-release-version TAG=vX.Y.Z` validates that contract.

The tag workflow builds five native Python wheels, one source distribution,
and one npm tarball. Those files are uploaded once, then a common
`release-ready` gate downloads and inspects the exact bytes, verifies the
cross-platform inventory and package metadata, performs clean-install smoke
tests, and generates `SHA256SUMS`, `release-manifest.json`, and an SPDX 2.3
artifact manifest. Only that validated bundle may cross a publication boundary.
Per-tag concurrency and immutable, attempt-specific workflow artifacts prevent
parallel or resumed runs from silently mixing outputs. An existing GitHub
release is never overwritten.

Registry publishing authenticates with API tokens stored as `release`
environment secrets (`NPM_TOKEN`, `PYPI_TOKEN`), so every publisher runs behind
that environment's required review; a token is unreachable from any other job,
and the read-only build and rehearsal graph may not reference one at all.
The npm job keeps job-scoped `id-token: write` so it can still emit provenance,
which npm generates from the OIDC token independently of how we authenticate.
PyPI gets no OIDC scope: PEP 740 attestations only work via Trusted Publishing,
so under token auth they are unavailable and requesting the scope would be unused
privilege. A
production tag requires npm publishing to be explicitly enabled; PyPI is
independently gated. Every enabled registry package is downloaded again,
hash-matched to the sealed manifest, and clean-consumer tested before CI
publishes the final GitHub release. Each registry is gated only by its
`*_PUBLISH_ENABLED` repository variable, so a tag publishes wherever those are
`true` when it runs — check them first. See
[`docs/releasing.md`](docs/releasing.md) for the activation checklist,
publisher bootstrap, approval boundary, and partial-publication recovery
procedure.

Release and packaging changes also run the same job graph in non-publishing
rehearsal mode. The rehearsal builds and clean-installs every exact artifact,
exercises the npm tarball in Node and Firefox, and verifies the checksums,
manifest, SBOM, and intended GitHub release inventory without requesting
registry or deployment authority.

## L-USDT examples

Executable Rust examples for the complete L-USDT/BTC pair workflow are in
[`examples/`](examples/README.md):

```bash
cargo run --example lusdt_submarine
cargo run --example lusdt_reverse
```

The submarine example covers validated L-USDT funding for a BTC Lightning
invoice. The reverse example covers response validation and the wallet-neutral,
caller-funded PSET claim flow required to receive L-USDT while paying Liquid
fees in L-BTC.

The L-USDT asset id is never taken from the maker. `LiquidAssetContext::trusted_lusdt`
resolves it from the canonical Liquid USDt on mainnet, or from a caller pin
(`lusdt_asset_id` on the binding requests, `LUSDT_ASSET_ID` in the examples) on
testnet and regtest, and only cross-checks the maker's pair card against it. A
create response is then validated against that trusted context.

## USDT-RGB wallet integration

The Rust core, UniFFI/Python and wasm/TypeScript expose RGB-on-Bitcoin submarine
and reverse swaps. The caller pins the RGB contract when creating the swap.
The SDK validates the HTLC and prepares/signs claim or refund PSBTs; the caller's
rgb-lib wallet transfers the asset, validates consignments, funds fees if needed,
colors the PSBT and completes its wallet operation after broadcast.

See [the RGB design and integration status](docs/rgb-swaps-plan.md),
[Python bindings](bindings/python/README.md#usdt-rgb-swaps) and
[TypeScript bindings](typescript-sdk/README.md#usdt-rgb-swaps). The
[Python](bindings/python/examples/rgb_spend.py) and
[TypeScript](typescript-sdk/examples/05-rgb-spend.ts) examples demonstrate the
wallet adapter boundary. Live rgb-lib regtest validation is recorded in [the native example](examples/rgb-regtest/README.md).

## Atomic BTC <-> USDT-RGB swaps on L1

The maker also serves single-transaction atomic swaps between on-chain BTC and
USDT-RGB (`BTC/USDT-RGB`, `USDT-RGB/BTC`) over `/v2/swap/atomic/*`. These are
not Boltz chain swaps: one transaction moves both legs, with no HTLC, no
timelock and no refund. Either it confirms and both sides are paid, or it never
reaches the chain.

The SDK quotes, verifies and tracks; the taker's rgb-lib wallet signs. The
offer, request, proposal and completion are rgb-lib messages passed through as
JSON:

1. `quote` — the quote is verified against the request and your
   `expectations` (asset id, network, fee ceilings, `minToAmount` /
   `maxFromAmount`), and the offer against the quote, before it is returned.
   Feed `quote.offer` to the wallet's `accept_swap_offer`.
2. `request` — posts the wallet's `OnchainSwapRequest` and returns the maker's
   unsigned proposal. It takes the wallet's coins (rgb-lib's `list_unspents`)
   and refuses a proposal that lists any of them as the maker's. Sign it with
   `complete_swap_proposal`.
3. `complete` — posts the signed `OnchainSwapCompletion`; the maker signs last
   and broadcasts. Finish in the wallet with `accept_swap_transfers`.
4. `status` / `track` — poll until a terminal state (`transaction.claimed`
   means settled).

```ts
import { AtomicSwaps, ATOMIC_BTC_TO_USDT_RGB, SwapClient, init } from "@kaleidorg/swap-sdk";

await init();
const atomic = new AtomicSwaps(new SwapClient(makerUrl));

const quote = await atomic.quote(
  { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n, direction: "from" },
  { assetId, bitcoinNetwork: "Regtest", maxNetworkFeeSat: 10_000n, minToAmount },
);
// wallet: accept_swap_offer(quote.offer) -> requestJson
const { proposal } = await atomic.request(quote.id, requestJson, walletOutpoints);
// wallet: complete_swap_proposal(proposal) -> completionJson
const { txid } = await atomic.complete(quote.id, completionJson);
const final = await atomic.track(quote.id);
```

Amounts are in base units (sats, or USDT-RGB's 6-decimal units), not the pair
card's display units. `maxServiceFee` only bounds the fee the maker declares,
so set `minToAmount` (or `maxFromAmount` for a `to` quote) to bound the price.

The same flow is `kaleidorg_swap_sdk::atomic` with
`BoltzApiClientV2::atomic_pairs` / `atomic_quote` / `atomic_request` /
`atomic_complete` / `atomic_swap` in Rust, and `SwapClient.get_atomic_pairs` /
`atomic_quote` / `atomic_request` / `atomic_complete` / `get_atomic_swap` in
Python. The full walkthrough is
[`typescript-sdk/examples/06-atomic.ts`](typescript-sdk/examples/06-atomic.ts).
Wallets need an rgb-lib with kaleidoswap/rgb-lib#18. The live signet maker
ships atomic pairs disabled; use a regtest maker.

---

# The swap engine (Boltz protocol)

The engine builds a `one-time use and dispose wallet` for the following bitcoin script:

NORMAL (SUBMARINE) SWAP:

```
    HASH160 <hash of the preimage> 
    EQUAL
    IF <receiver public key>
    ELSE <timeout block height> 
    CHECKLOCKTIMEVERIFY
    DROP <sender public key> 
    ENDIF
    CHECKSIG
```

REVERSE SWAP:

```
    SIZE
    [32]
    EQUAL
    IF
    HASH160 <hash of the preimage>
    EQUALVERIFY <receiver public key>
    ELSE
    DROP <timeout block height>
    CLTV
    DROP <sender public key> 
    ENDIF
    CHECKSIG
```

This script captures the following spending conditions:

```
Either; a preimage and the receiver's signature is required // happy case (claimTx)
Or; after a timeout the senders signature is required. // dispute (refundTx)
```

The `receiver` will be able to claim the funds on-chain.
We are the receiver in case of a reverse swap; the swap service is in case of a normal swap.

The `sender` will be able to claim funds on LN, once the receiver claims the on-chain funds and reveals the preimage.
We are the sender in the case of a normal swap, and the swap service is in the case of a reverse swap.

## Procedure

There is no requirement for a database as we will not persist any data.

We simply create keys, build a script, generate a single address corresponding to this key, watch the address for
payment and spend the utxo by building a transaction, solving the spending conditions and broadcasting.
We do not need to store transaction history or address indexes etc. This has to be handled by the client.

The client must ensure that they are rotating the keys and preimages being used. There are helper structs and methods
for this (`SwapMasterKey`, `Preimage`).

In the case of `normal swaps`; in the happy case the swap service pays our invoice and claims the on-chain funds.
The client (us) will ONLY be required to create the swap script and spend it in case the service cheats and we need to
claim back funds onchain from the script after a timeout.
We would be the `sender`; and can only spend after a timeout in case of a dispute.

In the case of `reverse swaps`; in the happy case, the client (us) will ALWAYS be required to build and spend from the
script to claim on-chain funds.
We would be the `receiver`, and the solution we have to create for the reverse swap is the `preimage` of a hash and a
signature from our key.

The standard procedure for a `reverse swap` happy case:

- Create a `keypair.{seckey,pubkey}`
- Create a random secret (preimage)
- Create `hash`=sha256(preimage)
- Share `keypair.pubkey` and `hash` with the swap service
- The service uses this to create the script on its end and sends it back as a `swap tree` along with
  an LN `invoice` for us to pay and an onchain `address` that it will fund for us to claim
- The service also returns its `pubkey` and the `timeout` used
- Verify the response and the preimage used in the invoice (the service cannot claim the invoice until the
  preimage is known)
- Build the script on our end using: `our_pubkey, hash, service_pubkey and timeout`
- Generate the address from the script and check for a match against the `address` provided
- Pay the `invoice`
- The service confirms the `invoice` paid and funds the `address`, creating a utxo that we can spend
- Construct a transaction to spend this utxo, solving hashlock + signature
- Sweep the utxo to your existing bitcoin wallet
- Once the utxo is spent, the preimage is publicly revealed and the service can claim the `invoice`

### Liquid

The procedure for liquid is the same as Bitcoin, with the addition of blinding logic associated with `Asset` and `Value`.

## Core Libraries/API

- [boltz](https://docs.boltz.exchange/v/api/api)
- [bitcoin](https://docs.rs/bitcoin)
- [elements](https://docs.rs/elements)
- [lightning-invoice](https://docs.rs/lightning-invoice/latest/lightning_invoice/)
- Chain data access
    - [electrum-client](https://docs.rs/electrum-client/latest/electrum_client/)
    - [esplora](https://github.com/blockstream/esplora/blob/master/API.md)

## WASM

This crate supports WASM.

When building for WASM, only `esplora` can be used for chain data access (`electrum` isn't compatible).

### Prerequisites

#### Building WASM

* Install [wasm-pack](https://rustwasm.github.io/docs/wasm-pack/): `cargo install wasm-pack`
* (Mac Only) Install [llvm](https://llvm.org/): `brew install llvm@21` (the
  Makefile prefers the versioned `llvm@21` keg and falls back to unversioned
  `llvm`) — needed to cross-compile secp256k1's C to wasm

#### Testing WASM

* Install the default testing browser [Firefox](https://www.mozilla.org/en-US/firefox/)
* When testing on Safari:
    * Enable safaridriver (might need sudo): `safaridriver --enable`
* When testing on Chrome:
    * Install chromedriver: `brew install chromedriver`

    + (Mac Only) Allow use of chromeriver if the first run fails:
      ` Settings > Privacy & Security > Allow use of chromedriver`

## Tests

The best place to start diving into this repo is the `tests` directory. This contains integration tests for bitcoin and
liquid, with complete examples of library usage.

Run all tests, except the ones that require a local docker regtest environment:

```bash
make cargo-test
```

### Regtest tests

To run the complete regtest integration tests you first need to start the regtest environment (requires initializing
git submodules):

```bash
./regtest/start.sh
make cargo-regtest-test
./regtest/stop.sh
```

### Testing WASM

Tests can be run in the browser using WASM. By default Firefox is used, but there are alternative make targets for
Chrome and Safari.

```bash
make wasm-test # runs tests that don't require the regtest environment

make wasm-regtest-test # runs regtest tests (requires the regtest environment)
```

## Assumptions

This library makes the following assumptions:

- A reverse swap has one designated HTLC input. Discovery may return multiple UTXOs, but the SDK selects only the
  output matching the exact script, asset, amount, and expected transaction id when one is available.

- Bitcoin reverse swap sweep/drain is 1 output

- The legacy single-input Liquid claim/refund path is L-BTC-only: one payout output and one explicit policy-asset fee
  output. The payout is confidential when the destination address carries a blinding key and explicit otherwise, except
  that a confidential HTLC cannot be swept to an explicit destination — its input blinding factors would have no
  blinded output to balance against. Magic routing is also L-BTC-only.

- Liquid HTLC discovery validates the exact script, asset, and transaction. Claims and all L-USDT spends also require
  the exact expected amount; legacy L-BTC refunds intentionally reclaim the positive amount actually locked so an
  underpayment remains refundable. L-BTC HTLCs may be confidential with their matching blinding key or explicit
  without one; native L-USDT HTLCs are explicit and must not include a key. L-USDT spending uses the caller-funded
  PSET flow because its Elements fee must be paid separately in L-BTC.

- Caller-funded PSET finalization is offline. The wallet must source real, spendable Liquid inputs; the SDK validates
  their commitments and any supplied full previous transactions, but cannot prove chain inclusion without a backend.

# Acknowledgment

This repository is a fork of [boltz-rust](https://github.com/SatoshiPortal/boltz-rust),
developed and maintained by Bull Bitcoin (www.bullbitcoin.com) — the swap engine
at the core of this SDK is their work (MIT licensed).

Upstream declares MIT in its `Cargo.toml` but ships no `LICENSE` file and no
copyright notice, so there is no upstream notice to reproduce verbatim. Our
[`LICENSE`](LICENSE) therefore records the attribution on their behalf, as a
stacked copyright line covering the boltz-rust contributors alongside
KaleidoSwap. The year span is derived from the upstream commits carried in this
repository's history; the authoritative contributor list is the upstream
repository itself.

Special thanks (from the upstream project) to:

- [michael1011](https://github.com/michael1011) for guidance on implementation and swaps
- [stratospher](https://github.com/stratospher) for contributions and pairing through liquid
  confidential transactions
- [RCasatta](https://github.com/RCasatta) for guidance on liquid

---

Product overview on the dedicated [website page](https://kaleidoswap.com/products/sdk).
