# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

This release turns the `boltz-rust` fork into the foundation of the **KaleidoSwap
SDK**: it exposes the swap engine through UniFFI and WebAssembly bindings and
renames the crate family accordingly. The Boltz swap engine (scripts, MuSig2,
PSBT construction, key derivation) is kept as-is — it is the valuable,
hard-to-rewrite core we are building on.

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
