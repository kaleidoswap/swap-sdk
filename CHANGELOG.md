# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed — reverse swap creation sends the caller's `pairHash`

`CreateReverseRequest` carried no `pair_hash` while `CreateSubmarineRequest`
and `CreateChainRequest` both did, so serde dropped the field and the maker
received a reverse create with no rate lock. It then created the swap at
whatever the rate happened to be.

The maker enforces the lock correctly once it arrives — a wrong hash comes back
`pair_hash_mismatch`, a malformed one `invalid_length`. Neither check could run
for reverse. Measured against a live maker, the same call per swap type with a
well-formed but deliberately wrong hash:

| type | result before |
|---|---|
| submarine | `pair_hash_mismatch` |
| chain | `pair_hash_mismatch` |
| reverse | swap created |

**Callers who were passing a stale or wrong `pairHash` on a reverse create and
succeeding will now be rejected.** That is the point of the field, but it is a
visible change: the rejection is new, not the staleness. It matters most on
pairs that re-price on every call, which is exactly when the lock is load
bearing.

### Fixed — a swap status keeps the maker's history and failure fields

`GetSwapResponse` modelled only `status`, `zeroConfRejected` and `transaction`,
so serde discarded everything else the maker sends:

```json
{"events":[{"kind":"invoice_issued","ts":1786704659}],
 "failureDetails":null,"failureReason":null,
 "id":"01KZZYB138E7C3HZX7Q1YBGAQG","paymentStatus":"pending",
 "status":"swap.created","type":"reverse"}
```

Callers got `status` alone — no history to see what a swap had done, and no
`failureReason`/`failureDetails` to say why it stopped. Anything needing the
timeline had to bypass the SDK and query the maker directly.

Now carries `id`, `type`, `paymentStatus`, `failureReason`, `failureDetails`
and `events` (a new `SwapEvent { kind, ts }`). All optional: Boltz serves none
of them, so those responses still parse, and an absent field stays absent on
the way back out rather than appearing as a null.

### Added — `isConnected()` on the WebAssembly binding

`runWsLoop` reconnects rather than returning, which is right for a long watch
but leaves a dropped socket invisible from JS: updates simply stop arriving,
indistinguishable from a swap with nothing to report, and the loop's promise
never settles because it is not a failure signal. `isConnected()` is the bit
that tells those apart.

### Fixed — `is_connected()` reports the socket rather than the loop

It tested whether `restart_sender` was `Some`. That sender is installed at the
top of *every* connection attempt, before the dial, and reinstalled on each
retry — so through an outage, with the loop cycling failed dials and
`reconnect_delay` sleeps, it answered `true` throughout.

It now tracks an explicit flag: cleared while dialling or retrying, set only
while a socket is established, cleared again when the loop exits.

**Existing Rust callers see different values.** Anything reading it as "the
loop is running" will now read false during an outage — which is the answer the
name promises, and the reason the previous value was not usable for detecting
one.

## [0.3.0] - 2026-08-13

### Added — Arkade Intents venue (`@kaleidorg/swap-sdk/arkade`)

An optional subpath serving the Arkade Intents RFQ routes — `arkade:BTC ↔
lightning:BTC` plus the intra-Arkade asset-swap covenant — against any solver
card. It is opt-in at the bundler level: `@arkade-os/sdk` and `@arkade-os/swap`
are optional peer dependencies resolved only when the subpath is imported, so a
Boltz-only consumer never pays for the Arkade dependency graph. The imports are
static because MV3 service workers forbid dynamic `import()`.

`ArkadeIntentsVenue` exposes prepare / notifyFunded / claimReceive / refundSend
for the corridors, prepareAssetSwap / notifyAssetSwapFunded / cancelAssetSwap
for the intra-Arkade offers, and one resumable `reconcile()` the host drives
from its own scheduler — the venue owns no timers, which is what lets an MV3
popup close mid-swap.

Two properties are load-bearing rather than incidental:

- **Every recovery record is persisted before value moves.** A corridor record
  is written before the lockup is funded, and it is plain JSON end-to-end —
  including the serialized `VHTLC.ScriptV2` options — so any store can hold it
  and the covenant rebuilds byte-identically after a restart.
- **Every terminal transition is decided from chain evidence**, never from a
  local flag or a relay status message. A prepared send whose lockup is live
  self-heals to funded rather than being cancelled (the host may have broadcast
  and crashed before reporting), a receive past its refund horizon settles when
  the solver's claim daemon claimed it, and a timeout means unknown, not failed.

Asset swaps never expire: an unfilled offer stays open until a solver fills it
or the user cancels, and cancellation races a fill — a race lost to the solver
is reported as fulfilled, which is a success rather than an error.

### Breaking — map-valued responses cross to JS as plain objects, not `Map`

The change below alters the shape of values already in callers' hands, so the
next release is a minor bump rather than a patch. It is confined to the
WebAssembly binding: the Rust crate and the Python distribution never went
through this serializer and are unaffected.

`to_js` built its `serde_wasm_bindgen::Serializer` without
`serialize_maps_as_objects`, so the JS shape of a response followed the Rust type
that produced it — a struct became a plain object, a `HashMap` became a `Map`.
The pairs and nodes responses are a struct wrapping a `HashMap<String, _>`, which
put the transition *inside* a single response, with nothing in the shape to mark
where:

```js
pairs.BTC            // struct field — a property read
pairs.BTC["L-BTC"]   // HashMap entry — undefined; needed .get("L-BTC")
```

Property access on a `Map` returns `undefined` rather than throwing, so this
failed silently, and TypeScript could not catch it: these payloads are Rust-defined
and typed `any`. `submarinePairs`, `reversePairs`, `chainPairs`, and `nodes` are
the affected responses. Every map crossing the boundary is keyed by `String`, so
the conversion loses nothing, and a uniform object shape is what this binding's
docs already claimed.

Callers who worked around the old shape with `.get(key)` must switch to property
or index access. Callers who wrote what the types promised now work.

### Fixed — `derivePreimage` returns the object its declared type describes

`derivePreimage` serialized a `serde_json::json!` value, whose object variant is
a map, so it reached JS as a `Map` while its neighbour `deriveSwapKey` — built
from a real struct — arrived as a plain object. Against the `DerivedPreimage`
interface the TS SDK declares, `preimage.sha256` type-checked and read
`undefined` at runtime.

This one was the most expensive to diagnose: `sha256` is the preimage hash passed
as `preimageHash` when creating a reverse or chain swap, so the lost value
surfaced as the maker rejecting the request for a missing field, several steps
away from the SDK that dropped it. It is now a named `DerivedPreimage` struct
mirroring `DerivedKey`; nothing about a preimage has dynamic keys. The serializer
change above fixes the shape either way, but the struct is what makes the
boundary type match the declaration on both sides.

Replacing the `json!` value also surfaced that `Preimage::bytes` is an `Option`
that `json!` would have quietly rendered as `preimage: null` against a declared
`string`. `from_swap_key` always populates it, so the case was unreachable; it now
returns an error instead of a null field.

### Breaking — every rejection from the WebAssembly binding is an `Error`

The change below alters the type of a value already in callers' hands, so it is
breaking for the WebAssembly binding; the Rust crate and the Python distribution
throw nothing across this boundary and are unaffected.

`js_err` threw `JsValue::from_str(...)`, so `catch (e)` yielded a bare **string**:
no `.message`, no `.stack`, `e instanceof Error` false, and invisible to the
`isKaleidoSwapError` narrowing this package documents. That covered every
deserialization failure and every hex/enum parse, while failures from the swap
engine were already proper `Error`s carrying a `code` — so the shape of a
rejection depended on how far into the call it got.

Every rejection produced after an argument reaches Rust is now a JS `Error` with
a stable `code`. Input the bindings reject themselves uses `InvalidArgument` and
names the offending argument or field; engine failures keep their own code and
binding-internal failures use `Internal`. Values rejected earlier by
wasm-bindgen's generated ABI glue — for example a `number` supplied where a
declared `bigint` is required — remain native JavaScript errors without an SDK
code. Key and preimage arguments name themselves too, since upstream parsers can
otherwise return messages that do not identify the offending argument.

Callers who compared a rejection as a string (`e === "unknown network: x"`, or a
`typeof e === "string"` branch) must read `e.message`, or branch on `e.code` via
`isKaleidoSwapError`. Callers who already used `isKaleidoSwapError` see strictly
more rejections through it than before. Note that `String(e)` now carries the code
as a name prefix (`InvalidArgument: unknown network: x`), matching how engine
errors have always stringified.

### Fixed — a mistyped argument is reported instead of trapping the module

Passing a non-string where the binding declares `string` trapped with
`RuntimeError: memory access out of bounds`, from a frame containing no Rust. It
reproduces on the published `@kaleidorg/swap-sdk@0.2.0`.

wasm-bindgen marshals a `String` parameter in its generated JS glue, *before* any
Rust code runs: `passStringToWasm0` reads `arg.length` and `arg.charCodeAt` and
hands the result to the wasm allocator. Given a non-string it computed a bogus
length and trapped inside the allocator. Passing arguments in the wrong order was
the ordinary way to reach it, and TypeScript cannot catch that for a plain-JS
caller — so the failure read as memory corruption when nothing was corrupt. The
argument never got as far as the function, which is also why it pre-empted the
error the request object itself would have produced: a malformed request now
rejects with ``missing field `from` ``, the message serde produced all along.

Every exported string parameter is now taken as an unconverted JS value and
checked in Rust, rejecting with ``argument `network` must be a string``. The
consumer-facing parameter declarations remain byte-for-byte unchanged — those
parameters are still declared `string`, and the check is what a plain-JS caller
gets in place of a trap. Internal wasm-bindgen declarations do change but are not
re-exported by the TypeScript SDK.

## [0.2.0] - 2026-08-11

The breaking change and the new binding below each make this `0.2.0` rather than
`0.1.2`, and the claim-amount fix adds a field to a public Rust struct, so the
crate has a compile-level break of its own. Every surface carries a change this
time: the claim-amount fix is in the swap engine, so it reaches the Rust crate
and the Python bindings as much as the WebAssembly build. The Rust crate, Python
distribution, and npm package share one public version, so all three move
together.

### Breaking — TypeScript `init()` accepts a narrower source type

`init` no longer takes wasm-bindgen's `InitInput`. It takes a hand-written
`WasmSource` (`BufferSource | URL | Request | Response | string`) — the same
union minus `WebAssembly.Module`, which TypeScript declares as an *empty*
interface. An empty interface is structurally assignable from any non-nullish
value, so its presence collapsed the union and `init(42)` typechecked. A caller
holding a pre-compiled module now uses the new `initWithModule` export. A
zero-emit type assertion in `src/index.ts` fails the build if the union is ever
widened back, since such a union still *looks* precise.

Callers who passed the object form (`init({ module_or_path: bytes })`) must pass
the bytes, or nothing at all — `init` wraps the argument itself, so the object
form double-wrapped and threw `WebAssembly.instantiate(): Argument 0 must be a
buffer source` at runtime. It typechecked for the same reason `init(42)` did.

### Fixed — Bitcoin claims enforce the amount the swap was created for

Liquid claims already required the swap HTLC output to hold exactly the agreed
amount. Bitcoin claims had no equivalent check: `BtcSwapScript` carried no
expected amount, UTXO selection returned the first output matching the script
pubkey, and the only value test on the claim path was that the output covered the
miner fee.

A counterparty that locked less than agreed therefore still received our
preimage, and we claimed whatever happened to be there. **That is not recoverable
after the fact** — publishing the preimage is what lets the counterparty take our
side of the swap, so a short lockup was paid for at full price. It affected chain
swaps claiming on Bitcoin and reverse swaps paying out to Bitcoin.

- `BtcSwapScript` gains an expected amount, populated from the same response
  fields the Liquid constructors already use: submarine `expectedAmount`, reverse
  `onchainAmount`, and the chain swap's `details` amount. It is a public field on
  a public struct that is not `#[non_exhaustive]`, so Rust callers that build one
  from a struct literal rather than from a swap response must supply it to
  compile.
- `BtcSwapScript::select_utxo` now applies the rule the Liquid one applies to
  claims — match on script pubkey, and on txid when one is supplied; require an
  exact amount; report a `Bitcoin swap amount mismatch` error rather than falling
  through to the first script match. Exact is exact in both directions, so an
  over-funded HTLC is refused as well: a lockup that does not match the swap it
  was created for is not one to spend a preimage against.
- Refunds keep the historical tolerance and recover whatever positive amount
  reached the correctly identified HTLC. No secret is at stake on that path, and
  refusing would strand the funds.

### Fixed — TypeScript `await init()` works in Node

`0.1.1` could not load its own WebAssembly from Node at all, and neither defect
was visible at compile time:

- **The packaged binary was unreachable.** Adding an `exports` map in `0.1.1`
  ended Node's legacy resolution, under which any subpath was fetchable, so
  `require.resolve("@kaleidorg/swap-sdk/vendor/bindings_wasm_bg.wasm")` failed
  with `ERR_PACKAGE_PATH_NOT_EXPORTED` — the path the `0.1.1` README told Node
  consumers to take.
- **The caller was made responsible for loading the SDK's own binary.** Rather
  than re-export `./vendor/*` — promoting a build directory to public API while
  still leaving the caller to resolve, read, and wrap — a `"node"` export
  condition selects a thin `dist/index.node.js` that reads the packaged binary
  itself. `await init()` is now correct in both runtimes, and a `file:` source
  passed explicitly (including the exported `wasmUrl`) is read from disk too
  rather than handed to a `fetch` that rejects it.

The browser entry is unchanged and still references no `node:` builtins, so
bundlers need no configuration — that is why this is a separate entry rather
than a guarded `await import("node:fs")` inside `init`. `"browser"` precedes
`"node"` in the `exports` map: Node never matches `"browser"`, so ordering it
first costs nothing there and keeps an isomorphic bundler that sets both
conditions from pulling `node:fs/promises` into a browser bundle.

`main` and `types` point at the node entry, and a new top-level `"browser"` field
points at the browser one. Both fields are read only by resolvers that skip
`exports` — legacy bundlers, `moduleResolution: node10`, some test runners — and
those are all Node-ish, so leaving `main` on the browser entry aimed the fallback
at the one build that cannot work in the runtime reading it. Browsers never
consult `main`; the pre-`exports` bundlers that do read `"browser"` instead, which
is why this needs both fields rather than a flip.

### Fixed — the npm package smoke test asserts what a consumer writes

The pack smoke test installed the tarball and imported by package name, which
was right, but it asserted `init(await readFile(wasmUrl))` — the workaround
rather than the call a consumer writes. It tested the ceremony instead of
catching it, which is why CI stayed green through both defects above. It now
asserts the zero-argument call, exercises each explicit source form
(`BufferSource`, `file:` URL, pre-compiled module) in its own process because
wasm-bindgen memoizes the instantiated module, and fails if the browser entry
ever grows a `node:` specifier. Verified in both directions: the published
`0.1.1` tarball fails the new assertion; this build passes it.

It also resolves the package through Node's own resolver twice — once with
default conditions, once with `--conditions=browser` added, standing in for an
isomorphic bundler that sets both — and asserts which entry each lands on, so
neither the `exports` map nor its condition order can regress unnoticed.

### Added — cooperative chain-swap claims reach the WebAssembly bindings

The core already supported cooperative chain claims, and the uniffi bindings
already exposed them through `TransactionOptions.chain_claim`, but the
WebAssembly bindings could not reach them: `TxParams` carries `cooperative` as a
bare bool, and the cooperative chain path additionally needs the lockup script it
signs against. So `constructClaim` documented `cooperative: false` as the only
option for chain swaps, and JavaScript consumers were stuck on the script spend —
the more expensive witness.

`SwapScript.constructCooperativeClaim(preimageHex, params, lockupScript,
refundKeysSecretHex)` exposes the MuSig2 keyspend. Both extra arguments are
positional and required rather than squeezed through the serde `params` object:

- **`lockupScript`** — the cooperative path partial-signs a temporary refund
  against the lockup-side script, which `TxParams` cannot carry.
- **`refundKeysSecretHex`** — that temporary refund is signed with the swap's
  **refund** key, not its claim key. A chain swap carries two independent keys
  (`CreateChainRequest` has both `claim_public_key` and `refund_public_key`), so
  defaulting one to the other would be the quiet kind of wrong: the partial
  signature is made under the claim key, the server rejects it, and the caller
  gets a MuSig error rather than "you passed the wrong key." Swaps derived from
  `SwapMasterKey` use one key for both sides and are unaffected either way.

`params.cooperative === false` is rejected rather than silently dropped, since
the previous documentation told chain-claim callers to set exactly that and
`with_chain_claim` forces it true.

`TxParams.cooperative`'s documentation is corrected too: it implied chain swaps
had no cooperative path at all, when refunds always had one — a cooperative
refund is co-signed by the server and spends with no locktime, so it never waited
for the timeout. Only claims were missing.

`constructClaim`'s own note is corrected with it. It told chain-swap callers that
the lockup script and refund key were something the params object "does not yet
carry", which read as the capability being unavailable rather than as living on a
different method. That note is what wasm-bindgen copies into the generated
`.d.ts`, so left alone this release would have shipped types telling consumers the
cooperative chain claim does not exist, from the same package that exposes it. It
now names `constructCooperativeClaim` and describes `constructClaim` as the
script-spend path, matching what the hand-written TypeScript wrapper already said.

### Added — the browser entry is now gated on every pull request

`scripts/smoke-browser-package.mjs` required a tarball path, so it could only run
from a release workflow and no pull request ever loaded the browser entry in a
browser. It now packs a throwaway tarball when given no argument, matching
`smoke-package.mjs`, which makes the new `smoke:browser-package` script
self-contained; `package.yaml` runs it on the same runner image the release build
already uses for it.

`@types/node` is a new dev dependency, for the Node entry point. It is also why
the pack smoke test now greps the browser entry for `node:` specifiers: with Node
types ambient, a stray `node:fs` import in the shared source typechecks cleanly.

### Changed — package documentation

- **The npm and PyPI landing pages no longer describe an unpublished SDK.** Both
  registries carry `0.1.1`, so the Python description drops the "public PyPI
  publishing is not enabled for this release yet" preamble and the
  `pip install ./kaleidorg_swap_sdk-0.1.0-<platform>.whl` instruction in favor of
  `pip install kaleidorg_swap_sdk`. Install commands are no longer
  version-pinned, since a pin baked into a published description is stale the
  moment the next version ships.
- **The npm README documents the package that actually exists.** It predated the
  distribution rename and the RLN removal: it titled itself `@kaleidoswap/sdk`,
  installed `@kaleidoswap/sdk@0.1.0`, and its only usage example imported an
  `RlnClient` the package does not export. It now covers the real
  `@kaleidorg/swap-sdk` surface — `BoltzClient`, `BoltzWsApi`, `SwapScript`,
  `SwapMasterKey`, `isKaleidoSwapError` — and states the maker contract
  `BoltzClient.forNetwork` enforces: `"signet"` and `"regtest"` resolve, while
  `"mainnet"` and `"testnet"` are rejected rather than falling back to a
  third-party maker. Every snippet was executed against the published `0.1.1`
  tarball.
- A registry description cannot be revised in place, so both pages keep serving
  the `0.1.1` text until the next release publishes.
- The npm README now documents `await init()` as the zero-argument call it has
  become, replacing the "Node usage" section that told Node consumers to read
  `wasmUrl` and pass its bytes. The root README's runtime column said the same.
- **The npm package points at the repository it is built from.** `repository`,
  `homepage`, and `bugs` still named `kaleidoswap/kaleidoswap-sdk`, which the
  repository rename to `kaleidoswap/swap-sdk` left serving only a redirect.
  `publishConfig.provenance` is on, and npm checks `repository` against the
  repository the build ran in, so a stale value there is a publish-time risk
  rather than a cosmetic one. (The npm scope is `@kaleidorg`; the GitHub
  organization is `kaleidoswap` — different namespaces, as the 0.1.0 entry on the
  `kaleidorg` rename notes.) Equivalent references elsewhere are corrected in the
  entry below.

### Changed — repository identity and the release checklist

The npm manifest was corrected above; every remaining reference to
`kaleidoswap/kaleidoswap-sdk` is corrected here, so nothing this release ships
points at a name the rename left serving only a redirect.

- **The release SBOM's `documentNamespace` is the one that mattered.** It is a
  paired constant: `assemble_release.py` writes it and `verify_release_bundle.py`
  asserts it byte-for-byte, so the two had to move together or every release
  bundle would fail its own verification. `0.2.0` is the first release whose SBOM
  identifies the repository it was built from.
- The four Rust manifests (`Cargo.toml`, `macros`, `bindings`, `bindings-wasm`),
  the Python project metadata's `Repository` and `Issues` URLs, the git-dependency
  snippets in both READMEs, the Python README's example links, and the `gh`
  invocations throughout `docs/releasing.md` all move to `kaleidoswap/swap-sdk`.
  Historical changelog entries keep the old name: they are a record of what was
  true when written.

The release checklist is no longer written for one version. It was titled
"v0.1.0 activation checklist" with `0.1.0` substituted into every command, which
is the kind of document that is silently wrong by its second use. It now derives
`VERSION` from `release_version.py current` — the same value the release
preflight reads, and one that fails loudly if the six version sources disagree —
and takes `TAG` from it, so the commands are correct for whatever is committed
rather than for whatever was current when the doc was written.

Three statements in that file had also gone stale and are corrected: the
synchronized-version contract no longer claims the internal crates sit at a
specific version, the Rust git-dependency example pins the latest published tag,
and the PyPI section reflects that `kaleidorg-swap-sdk` now holds `0.1.0` and
`0.1.1` rather than being unclaimed.

## [0.1.1] - 2026-08-05

**The first release available on both registries.** There are no library
changes: every published artifact is functionally identical to 0.1.0.

0.1.0 reached PyPI but never reached npm. The release workflow invoked
`npm publish release-artifacts/*.tgz`, and npm reads a bare `a/b` argument as
a GitHub `owner/repo` shorthand rather than a file — so instead of uploading
it tried to clone a repository named after the tarball and exited 128. PyPI
had already published, because `publish-npm` and `publish-pypi` both depend
only on `release-ready` and therefore run concurrently. `0.1.0` is
permanently claimed on PyPI and absent from npm; install `0.1.1` instead.

### Fixed — release engineering

- **`npm publish` is given an explicit file path.** The argument is now
  `./release-artifacts/*.tgz`. npm's package-arg parser only treats an
  argument as a file when it begins with `./`, `../`, `~/`, `/` or a drive
  letter; anything else that looks like `owner/repo` is resolved as a git
  dependency.
- **The release workflow linter rejects a publish argument npm would misread.**
  `release-rehearsal.yaml` deliberately has no publish step, so no rehearsal
  can ever exercise that line — only a real tag reaches it, and by then PyPI
  has shipped. The check therefore lives in `check_release_workflow.py`,
  which runs on every pull request, with a test that reintroduces the exact
  regression.

### Changed — release documentation

- `NPM_PUBLISH_ENABLED` and `PYPI_PUBLISH_ENABLED` are documented as ordinary
  repository variables read at dispatch time, not values fixed in the
  workflow. Reviewers on the `release` environment are the actual gate, admins
  can bypass it, and npm publication cannot be undone.
- The 0.1.0 changelog entry is dated to the day it was released rather than
  the day its notes were written.

## [0.1.0] - 2026-08-05

This release turns the `boltz-rust` fork into the foundation of the **KaleidoSwap
SDK**: it exposes the swap engine through UniFFI and WebAssembly bindings and
renames the crate family accordingly. The Boltz swap engine (scripts, MuSig2,
PSBT construction, key derivation) is kept as-is — it is the valuable,
hard-to-rewrite core we are building on.

### Changed — swaps-only publish surface + KaleidoSwap maker defaults

Phase 0 of the SDK architecture plan: the published SDK is the **swap
protocol only**, pointed at **our** maker.

- **De-Boltz defaults**: `BoltzApiClientV2::default()` / `forNetwork()` now
  resolve to a KaleidoSwap maker and nothing else — **signet** →
  `maker.signet.kaleidoswap.com/v2`, regtest → the local harness. Networks we
  run no maker on **error**: **mainnet** until the mainnet maker is live, and
  **testnet** because signet is our testing network and no testnet3 maker
  exists. No default ever falls back to a third-party endpoint, so it cannot
  hand you a counterparty you did not choose. `boltz.exchange` remains
  reachable by name via an explicit `base_url`; the `BOLTZ_*` constants are
  kept.
- **New `Signet` network / `BitcoinSignet` chain**: the KaleidoSwap maker
  settles on [Mutinynet](https://mutinynet.com), so it needs a chain identity
  distinct from testnet3. Signet and testnet3 share an address encoding, so
  without a separate variant a signet maker paired with testnet3 chain access
  produced no error — just swaps funded and watched on the wrong chain.
  `Network::Signet` now maps to `BitcoinChain::BitcoinSignet` (Esplora default
  `https://esplora.signet.kaleidoswap.com`, KaleidoSwap's own index of the
  maker's chain — note the API is at the root, not under `/api`) and, on the
  Liquid side, to `LiquidTestnet`. Signet is therefore fully first-party: maker
  *and* chain access. Mainnet/testnet3 chain defaults stay public explorers, and
  Liquid has no KaleidoSwap explorer yet.
  `ElectrumBitcoinClient::default` **errors** for signet: Mutinynet publishes
  no public Electrum server, and a vanilla-signet server would silently serve a
  different chain. `parse_network` accepts `"signet"` in the wasm bindings, and
  the TypeScript `Network` union gains `"signet"`.
  **Binding consumers must regenerate**, not just recompile: the new enum
  variants extend the UniFFI wire surface. They are appended, so existing
  variant indices keep their meaning, but a generated module older than the
  library will not know `SIGNET`/`BITCOIN_SIGNET` exists.
- **Identity**: crate/wheel version `0.4.1` → `0.1.0`, KaleidoSwap
  description/authors; npm package renamed `@kaleidoswap/sdk` →
  **`@kaleidorg/swap-sdk`**. Fork provenance stays acknowledged in the README.

### Changed — package identity migrated to `kaleidorg`

Every published surface now shares one identity, at `0.1.0`:

| Surface | Was | Now |
| --- | --- | --- |
| Rust crate | `kaleidoswap-sdk` | `kaleidorg-swap-sdk` |
| Rust lib / Python import | `kaleidoswap_sdk` | `kaleidorg_swap_sdk` |
| Proc-macro crate | `kaleidoswap-sdk-macros` | `kaleidorg-swap-sdk-macros` |
| Python distribution | `kaleidoswap_sdk` | `kaleidorg_swap_sdk` |
| npm package | `@kaleidoswap/sdk` | `@kaleidorg/swap-sdk` |

The native library basename follows (`libkaleidoswap_sdk.*` →
`libkaleidorg_swap_sdk.*`), as does `uniffi.toml`'s `cdylib_name`. The GitHub
repository is **not** renamed, so `repository`/`homepage` URLs are unchanged.

This also **clears the public PyPI blocker**. The old normalized project
`kaleidoswap-sdk` already holds `0.1.0`–`0.5.6`, which is why production PyPI
publishing was hardcoded off; `kaleidorg-swap-sdk` is unclaimed, so `0.1.0` is
publishable under the new name. Publishing stays off by default, but it is now a
configuration decision rather than a technical constraint.

### Breaking — TypeScript `init()` signature

`init(input?)` narrowed from `Parameters<typeof initWasm>[0]` to
`InitInput | Promise<InitInput>`, and now forwards it as
`initWasm({ module_or_path: input })`. This avoids wasm-bindgen's deprecated
positional form, but a caller who already passed the object form
(`init({ module_or_path: url })`) no longer typechecks — pass the URL, `Request`,
`Response`, or bytes directly instead. Callers who pass nothing are unaffected.

Node consumers must now read the new `wasmUrl` export and pass its bytes,
because Node's `fetch` will not load a `file:` URL.

### Changed — registry publishing uses stored API tokens

Publishing authenticates with `NPM_TOKEN` and `PYPI_TOKEN`, held as **`release`
environment secrets**, instead of OIDC trusted publishing. This is a deliberate
reduction in the previous "no long-lived registry credential" property, taken so
a release does not depend on registry-side trusted-publisher bootstrap.

The invariant that replaces it is narrower but still enforced by
`scripts/check_release_workflow.py`:

- only `NPM_TOKEN` and `PYPI_TOKEN` may be referenced — any other secret name is
  rejected;
- a token is only reachable from a job that declares `environment: release`, so
  publishing still requires that environment's review;
- the read-only build and rehearsal workflows may not reference a token at all;
- `username:` is rejected, so authentication cannot silently become basic auth.

`id-token: write` is retained on the **npm** job only. npm generates provenance
from the OIDC token independently of how we authenticate, so it survives the move
to token auth. PyPI's PEP 740 attestations do not: the PyPA action ignores
`attestations: true` whenever a password is set, because attestations require
Trusted Publishing. That input is therefore set explicitly to `false` rather than
left at its `true` default, and the PyPI job requests no OIDC scope — asking for
one would be unused privilege, and claiming attestations we do not produce would
be worse.

The npm job now runs `npm whoami` before publishing. A bad credential fails on a
read-only call rather than part-way through an irreversible publish.

### Added — public PyPI publishing

`PYPI_PUBLISH_ENABLED` becomes a repository variable (default `false`) with real
`publish-pypi` and `verify-pypi` jobs, rather than a hardcoded `"false"`. The
distribution rename cleared the name collision that made this impossible; the
flag still defaults off, so a tag fails closed until it is set deliberately.

### Release engineering

- Reset the synchronized Rust, Python, and TypeScript public release line to
  `0.1.0`.
- Add commands to display, synchronize, and validate every public package
  version and lockfile.
- Validate version consistency in pull-request CI before release automation is
  enabled.
- Document the public PyPI name/version collision that the `kaleidorg` rename
  resolved, and keep production PyPI off by default until deliberately enabled.
- Package Python bindings with Maturin/UniFFI as platform-tagged native wheels
  instead of embedding native libraries in a universal Hatch wheel.
- Include complete Python distribution metadata, license, and classifiers.
- Harden the npm package manifest and contents, document the browser-first
  runtime contract, and add a clean-consumer WASM initialization smoke test.
- Add pull-request packaging CI for five native Python wheel targets, source
  reconstruction from the sdist, and clean artifact installation.
- Add locked TypeScript lint, formatting, unit-test, build, audit, tarball
  inspection, and consumer-install checks.
- Commit the platform-independent UniFFI Python glue fallback and reject
  generated drift in CI.
- Coordinate tag releases through one immutable bundle containing five native
  Python wheels, one source distribution, and one npm tarball, plus checksums,
  a release manifest, and an SPDX artifact SBOM.
- Publish npm and PyPI artifacts behind the protected
  `release` environment; see the token note above for how they authenticate.
- Exercise the exact production artifact graph in a read-only rehearsal,
  including clean Node and Firefox consumers and deliberate preflight, npm, and
  wheel-inventory failures.
- Download enabled registry packages after publication, require their bytes and
  inventories to match the sealed release manifest, and repeat clean-consumer
  smoke tests before publishing the final GitHub release.

### Added — WebAssembly / TypeScript bindings (`bindings-wasm` + `typescript-sdk`)

Browser-facing SDK, mirroring the Python surface for the web.

- **Separate `bindings-wasm` crate (wasm-bindgen).** UniFFI does not target
  browsers, so the web path uses `wasm-bindgen` in its own crate (it cannot
  cleanly share the UniFFI `bindings` crate — different export mechanisms,
  different runtimes). Exposes the swap
  key-management surface (`WasmSwapMasterKey`: BIP85 derivation, per-swap keys,
  deterministic preimages), `BoltzClient` (the Boltz swap API: pairs/fees,
  create submarine/reverse/chain swaps, status lookups, quotes, swap-restore),
  the client-side transaction surface (`SwapScript`: reconstruct submarine/
  reverse/chain scripts and build claim/refund txs; `BtcLikeTransaction`:
  hex/txid/broadcast), and the WebSocket swap-status stream (`BoltzWsApi` +
  `BoltzWsUpdates`). Because wasm-bindgen async methods can't hold
  `&ExportedType` borrows across the await, `constructClaim`/`constructRefund`/
  `broadcast` take primitives + a params object and rebuild the chain/boltz
  clients internally; `runWsLoop` is a sync method returning a Promise (clones
  the inner `Arc`) so the never-resolving loop doesn't hold a `&self` borrow that
  would block other calls on the object.
- **JS-object boundary via `serde-wasm-bindgen`** — swap values cross to JS as
  plain objects (typed `any` at the raw wasm layer), typed on the TS side by the
  hand-written interfaces. Async methods map to JS Promises.
- **`typescript-sdk`.**
  - `src/index.ts` — a hand-written typed wrapper that restores the domain types
    onto the wasm client's `any` boundary (`SwapScript`, `SwapMasterKey`), so TS
    callers get a fully-typed API. `tsc --noEmit` passes.
- **Build (`make wasm-pack-build`).** `wasm-pack`
  emits the JS package + `.d.ts` under `bindings-wasm/pkg/`. The secp256k1 C is
  cross-compiled to wasm via a wasm-capable clang; `CLANG_PREFIX` now prefers the
  versioned `llvm@21` keg and falls back to unversioned `llvm`.

Remaining typing note: the Boltz swap DTOs, `SwapStatus`, and `TxParams` have no
OpenAPI spec, so `BoltzClient` payloads, WS `next()` results, and
`constructClaim/Refund` params are typed `any` in the raw wasm `.d.ts` (the TS
SDK adds a hand-written `TxParams` interface; fully typed Boltz DTOs would need a
schema-generation step such as schemars).

### Changed — CI & docs

- **CI triggers fixed**: workflows ran on pushes to `master`, which doesn't exist
  (default branch is `trunk`) — no CI had ever run on this repository. Push
  triggers now target `trunk`; the `build-as-wasm-dependency` job's
  `cargo add boltz-client` updated to the renamed `kaleidoswap-sdk`.
- **Python formatting gate**: `make check-python` (run by the lint workflow) was
  failing on `tests/bindings/chain.py`; reformatted.
- **README rewritten** for the KaleidoSwap SDK: repository structure and
  bindings overview; the Boltz-protocol swap-engine documentation is
  kept (still accurate) under its own section, with fork provenance and upstream
  acknowledgment preserved. `bindings/` READMEs and the Python package metadata
  (description, URLs) updated from Boltz to KaleidoSwap.

### Changed — crate rename `boltz-client` → `kaleidoswap-sdk`

Mechanical, non-functional rename of the crate identity:

- Root crate/lib `boltz-client`/`boltz_client` → `kaleidoswap-sdk`/`kaleidoswap_sdk`.
- Proc-macro crate `boltz-client-macros` → `kaleidoswap-sdk-macros`.
- Bindings lib + `cdylib_name`, Python package/module, and native lib basename
  (`libboltz_client.*` → `libkaleidoswap_sdk.*`) all follow.
- The Boltz *protocol* surface is intentionally untouched: the `boltz` module,
  `BoltzApiClientV2`, and the `boltz.exchange` URLs remain (repointing those to
  the KaleidoSwap maker is a separate, functional change). The
  `SwapTransactionParams` Boltz-client field is named `boltz_api` — the mechanical
  token sweep had briefly caught it; it's restored to a protocol-accurate name.

### Fixed

- **`macros` dependency now resolves by `path`.** [0.4.0] moved `macros` to the
  published `boltz-client-macros` on crates.io (no path). The renamed
  `kaleidoswap-sdk-macros` is not published, so the root dependency now points at
  the local `macros/` member (`path = "macros"`), making the workspace
  self-contained.

Addressing PR review (Codex):

- **Wasm `BoltzClient` validates create-swap responses.** `createSubmarineSwap`/
  `createReverseSwap`/`createChainSwap` now run the same `validate(...)` checks as
  the native bindings (they take a `network` arg for this), so a mismatched lockup
  address/tree is rejected before the caller funds it.
- **Python call sites use the `boltz_api` keyword.** The examples/binding tests
  still passed `kaleidoswap_sdk=` after the field rename; updated to `boltz_api=`.
- **The TS package bundles the wasm output** (`typescript-sdk/vendor/`, populated
  by `make wasm-pack-build`) and imports it via an in-package path, so a published
  `@kaleidoswap/sdk` resolves without escaping the package.
- Chain-swap **cooperative** claims over wasm require `cooperative: false`
  (documented on `TxParams`); the cooperative chain path needs lockup-script +
  refund-key options the wasm params object does not yet carry.

Second review round:

- **Invoice-form reverse swaps are validated too.** The previous fix only
  validated when `preimage_hash` was set; the invoice form (`preimage_hash`
  unset) skipped it. `createReverseSwap` now derives the payment hash from the
  invoice (`Preimage::from_invoice_str`) and always validates, erroring rather
  than returning an unvalidated response.
- **Vendored wasm imports use the `.js` extension.** As native ESM, the emitted
  `dist/index.js` keeps the relative specifier verbatim; `../vendor/bindings_wasm`
  is now `../vendor/bindings_wasm.js` so browser/Node ESM loaders resolve it
  without a bundler.

- **64-bit integers cross the wasm boundary as BigInt (lossless).** The wasm
  `to_js` serializer now uses `serialize_large_number_types_as_bigints`, so u64
  amounts up to u64::MAX are never rounded through an
  f64 — matching wasm-bindgen's own u64 ↔ `bigint` mapping in direct signatures
  and the convention of modern JS crypto libraries (ethers v6 / viem).
  Uniform rule: **every 64-bit integer field is `bigint` in JS.**
  - New `toJson` helper exported from the TS SDK (`JSON.stringify` throws on
    BigInt; the helper encodes bigints as decimal strings).
  - Requests accept `bigint` per the types; the boundary deserializer lifts both
    Number and BigInt. Python/native are unaffected (that path was already exact:
    serde_json prints u64 losslessly and Python ints are arbitrary-precision).

L-BTC / magic-routing round:

- **Explicit L-BTC HTLCs are accepted *and* spendable.** `validate_currency` no
  longer requires a blinding key for L-BTC, since KaleidoSwap Maker creates
  explicit L-BTC HTLCs and correctly omits `blindingKey`. The legacy
  single-input claim/refund builders were relaxed in the same change: they now
  source their funding secrets from the shared `unblind_swap_output` instead of
  demanding a blinding secret, so an explicit HTLC can actually be claimed or
  refunded. Without this the SDK would have accepted a swap it could not
  finish — a submarine user could fund a lockup and then be unable to build the
  timeout refund. The payout destination may now also be explicit. The one
  pairing that cannot work, and is rejected up front, is a confidential HTLC
  swept to an explicit destination: its input blinding factors would have no
  blinded output to balance against.
- **Caller-funded Liquid spends pin the payout address to the swap's chain.**
  `prepare_liquid_claim`/`prepare_liquid_refund` now parse `output_address` with
  the client's `LiquidChain` before any lookup or broadcast. `Address::from_str`
  alone accepts another network's encoding, so a wrong-network or mistyped
  address previously surfaced only after the hold invoice had been paid.
- **`parse_bip21` no longer panics, and is stricter (behavior change).** It
  previously indexed into split results and would panic on a URI with no `?` or
  a parameter with no `=` — a latent DoS on maker-controlled input. It now
  returns errors instead, and additionally rejects duplicate `amount`/`assetid`
  parameters, empty parameter values, and more than one query separator. This is
  an observable API change for a `pub` function: URIs that previously panicked
  or silently parsed now return `Error::Generic`.
- **Magic routing validates the whole destination, not just the asset.**
  `check_for_mrh` now checks the BIP21 scheme against the swap's chain, requires
  the address to be canonical and to belong to that chain, pins the policy asset
  per network, and rejects a zero amount. Bitcoin chains are covered too — the
  previous `_ => ()` arm meant a `liquidnetwork:` URI could be returned as the
  destination for a Bitcoin payment. Liquid regtest accepts both
  `liquidnetwork` and `liquidtestnet`, since Boltz and KaleidoSwap Maker
  disagree on the scheme for Elements regtest; the address params and policy
  asset still pin the chain.

### Dependencies

- New crate `bindings-wasm`: `wasm-bindgen`, `wasm-bindgen-futures`,
  `serde-wasm-bindgen`, `js-sys`.
- TypeScript package: `typescript` (dev).
- Tooling: `wasm-pack` + `llvm@21` (wasm build).

### Breaking

- The crate is now `kaleidoswap-sdk` (lib `kaleidoswap_sdk`); all
  `use boltz_client::…` imports become `use kaleidoswap_sdk::…`. The proc-macro
  crate is `kaleidoswap-sdk-macros`. The Python module is `kaleidoswap_sdk`.

## [0.4.1]

### Added
- `derivation_path` and `gap_limit` parameters on `post_swap_restore` and
  `post_swap_restore_index`. Pass `derivation_path = Some("m")` when the supplied
  xpub is already the swap-account key (`m/44/0/0/0`) so boltz derives
  `xpub/{index}` directly; omitting the path makes boltz apply its own default
  and match nothing.
- `invoice: Option<String>` field on `SwapRestoreResponse` (boltz returns it for
  submarine and reverse swaps).
- Tests for the swap-restore endpoints.

### Changed
- Boltz `regtest` submodule bumped to latest main.

### Dependencies
- Bumped `elements` from `0.25.0` to `0.26.2`.
- Bumped `lightning-invoice` from `0.32.0` to `0.34.0`.
- Bumped `electrum-client` from `0.21.0` to `0.25.0`.

### Removed
- The automated publish workflow.

### Breaking
- `post_swap_restore` and `post_swap_restore_index` gained `derivation_path` and
  `gap_limit` parameters; existing callers must pass them (`None, None` reproduces
  the previous behaviour).
- `SwapRestoreResponse` gained an `invoice` field; struct-literal construction of
  this type must be updated.

## [0.4.0]

### Added
- BOLT12 invoice support in `submarine_cooperative_claim`. New `LightningInvoice` enum (`Bolt11` / `Bolt12`) in `util::invoice`, plus a `util::bolt12::parse_bolt12_invoice` helper.
- `get_tx(txid)` on the `BitcoinClient` and `LiquidClient` traits, implemented for both Electrum and Esplora backends.
- Optional `transaction: Option<TransactionOut>` field on `ClaimDetails` and `RefundDetails`, plus a new `TransactionOut { id, vout }` struct, to support the extended swap-restore API response.
- Python bindings: `BtcLikeTransaction.hex()` and `BtcLikeTransaction.txid()`.

### Changed
- HTTP error reporting in `BoltzApiClientV2` unified across GET / POST / PATCH. Non-success responses now surface as `Error::HTTPStatusNotSuccess(StatusCode, Value)` carrying both status and the server-returned body (JSON or text), instead of `Error::HTTP(String)` with only the `error` field.
- `201 Created` responses are now treated as success.
- `macros` is published as `boltz-client-macros = "1.0.0"` on crates.io; the workspace no longer depends on it by path.
- Boltz `regtest` submodule bumped; submarine integration tests updated to cooperatively claim mainchain swaps (the new backend defers claims).
- CI now builds the language bindings.
- Removed the "early alpha" warning from the crate docs.

### Dependencies
- Added `lightning = "0.2.2"` (for BOLT12 parsing).
- Bumped `env_logger` from `0.7` to `0.11.8`.
- Locked `wasm-pack` version in tooling.

### Breaking
- `ClaimDetails` and `RefundDetails` gained a new field; struct-literal construction of these types must be updated.
- Boltz HTTP failures previously returned `Error::HTTP(String)`; they now return `Error::HTTPStatusNotSuccess(StatusCode, Value)`. The `Error::HTTP` variant still exists for other call sites, so callers that pattern-matched it for Boltz errors will silently stop matching.

## [0.3.1]

Baseline for this changelog. See git history for prior releases.
