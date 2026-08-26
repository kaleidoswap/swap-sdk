# kaleidorg-swap-sdk

**KaleidoSwap swap SDK** — client-side atomic swaps (Boltz protocol) across
Bitcoin, Lightning, and Liquid, for Rust, Python, and the browser.

The published surface is **swaps-only**: quote/create/watch a swap against the
KaleidoSwap maker, derive per-swap keys and preimages, and build the claim /
refund transactions client-side. Any Boltz-`/v2`-compatible endpoint works via
an explicit base URL.

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

## Partner attribution (organization API keys)

A partner organization can have the swaps it originates attributed to it, so the
volume and fees it drives show up in its own statistics. Attribution is opt-in
and needs an **organization API key** from the KaleidoSwap partner panel — a
`kld_test_…` key for signet and staging, `kld_live_…` for mainnet and
production. Without one, every client here behaves exactly as before and creates
unattributed swaps.

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
client = kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
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
plain HTTP is readable by anything on the path. Nothing renders the secret:
`Debug`, `__str__` and the JS getters show the key id and environment only. A
value that cannot be a key is rejected locally rather than reaching the maker as
a `401`, which is the same answer a revoked key gets.

**Server and native integrations only.** A key in a browser bundle is visible to
every visitor, who can then attribute their own swaps to — or exhaust the limits
of — an organization that is not theirs. Keep it in server-side configuration
and leave browser code on the unauthenticated client; a publishable attribution
key with allowed origins and per-key rate limits is a separate, later concept.

## Repository structure

| Path | What it is |
|---|---|
| `src/` | The swap engine (Boltz protocol): scripts, MuSig2, tx construction, key/preimage derivation, Esplora/Electrum chain access |
| `bindings/` | [UniFFI](https://mozilla.github.io/uniffi-rs/) bindings (Python today) |
| `bindings-wasm/` | wasm-bindgen bindings for the browser — same swap surface, 64-bit integers cross as `BigInt` |
| `typescript-sdk/` | TypeScript SDK (`@kaleidorg/swap-sdk`) wrapping the wasm package with hand-written types |
| `macros/` | Proc-macros (wasm-compatible `async_trait`, cross-target `test_all`) |

## Installation and supported runtimes

| Surface | Install | Supported v0.1.x runtime |
|---|---|---|
| Rust | `kaleidorg-swap-sdk = { git = "https://github.com/kaleidoswap/swap-sdk", tag = "v0.1.1" }` | Rust 1.88+, native and `wasm32-unknown-unknown` |
| Python | `pip install kaleidorg_swap_sdk` | Python 3.10+; wheels for Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64, sdist elsewhere |
| TypeScript | `npm install @kaleidorg/swap-sdk` | Browsers and Node 22+; `await init()` takes no argument in either |

Both registries are live as of `0.1.1`: `kaleidorg_swap_sdk` on PyPI (five
platform wheels plus an sdist) and `@kaleidorg/swap-sdk` on npm. The Rust crate
is not published to crates.io — depend on it by tag.

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
```

The generated fallback is committed. Pull-request CI regenerates it solely from
repository-pinned tool inputs and rejects any drift.

## Bindings

- **Python (UniFFI):** see [`bindings/`](bindings/README.md). Build with
  `cd bindings && make build-debug`; examples in `bindings/python/examples/`.
- **Browser/TypeScript (wasm):** `make wasm-pack-build` builds the wasm package
  and vendors it into `typescript-sdk/`. See `typescript-sdk/src/index.ts` for
  the typed surface (`BoltzClient`, `SwapScript`, `SwapMasterKey`,
  `BoltzWsApi`).

Pull-request packaging CI builds and clean-installs Python wheels for Linux
x86_64/aarch64, macOS x86_64/arm64, and Windows x86_64. It also reconstructs
the native package from the Python sdist and installs the packed npm tarball in
an isolated Node consumer.

## Releases

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
