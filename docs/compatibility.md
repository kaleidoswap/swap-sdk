# Maker and upstream Boltz compatibility

The SDK retains upstream Boltz compatibility through an explicit API base URL.
An exported SDK method does not establish that a particular maker serves its
required routes. The default KaleidoSwap maker supports a subset of Boltz's
public API and adds its own routes. This document records the deliberate policy
for the three route groups raised in maker issue #655.

The maker router was verified at commit
[`4ce72f57e2a5f2148b5c9f5ee30be289fb02bcbf`](https://github.com/kaleidoswap/kaleidoswap-maker-rs/blob/4ce72f57e2a5f2148b5c9f5ee30be289fb02bcbf/crates/maker-api/src/lib.rs).
These are source-level capability checks, not probes of a deployed maker.

| Capability | Rust core | WASM / TypeScript | UniFFI / Python / React Native | KaleidoSwap public maker |
| --- | --- | --- | --- | --- |
| Submarine server-claim co-signing | Raw GET/POST helpers and `SwapScript::submarine_cooperative_claim` | Not exported | Existing `SwapScript.submarine_cooperative_claim` helper; raw client methods are not exported | Required GET/POST claim routes absent |
| Maker-mediated raw Bitcoin/Liquid broadcast | `BoltzApiClientV2::broadcast_tx` | Not exported | Not exported | Required public transaction routes absent |
| Boltz-shaped BOLT12 offer management, invoice fetch and parameters | Five `BoltzApiClientV2` methods listed below | Not exported | Not exported | Required public BOLT12 routes absent |

Native method names above use the Rust/Python spelling; React Native generates
its language-specific names from the same UniFFI exports.

The policy is to **retain these existing Rust compatibility APIs and the existing
native submarine helper**, without expanding the raw client routes into other
bindings. No feature flag is needed: this preserves the current public API and
lets callers choose a compatible upstream server explicitly. Adding bindings
alone would not enable these operations against KaleidoSwap. Expanding maker
support requires a separate endpoint, authorization and protocol decision, then
tests against that backend and an updated matrix.

## Submarine claim co-signing

Rust's `get_submarine_claim_tx_details` and
`post_submarine_claim_tx_details` require `GET` and
`POST /v2/swap/submarine/{id}/claim`. The higher-level Rust/native
`submarine_cooperative_claim` verifies that the server's preimage pays the
caller's invoice and checks the counterparty key before signing the server's
claim. Keep these checks when using a compatible upstream server.

KaleidoSwap registers submarine **refund**, transaction and preimage routes, but
no submarine claim co-sign route. Its reverse and chain claim routes are
different protocols. The WASM `SwapScript.constructCooperativeClaim` constructs
a **chain** claim using the refund key and lockup script. Its native equivalent
is `construct_claim` with `TransactionOptions.chain_claim`; it is not the native
submarine co-sign helper. The binding parity alias checks that distinction.

## Public maker broadcast and chain-client broadcast

Rust's `BoltzApiClientV2::broadcast_tx` posts `{ "hex": ... }` to
`/v2/chain/BTC/transaction` or `/v2/chain/L-BTC/transaction`. Those public maker
routes are absent. The operator-only
`POST /v2/admin/liquid/tx/broadcast` requires operator authentication and is not
a public broadcast substitute. Operator tx-watch rebroadcast also acts on the
maker's saved transactions rather than arbitrary taker bytes.

Existing SDK chain-client broadcast remains available: WASM transactions use
`BtcLikeTransaction.broadcast` with an explicitly configured chain endpoint;
native callers use `ChainClient.broadcast_tx`. Rust supports its configured
Esplora/Electrum clients. Reconstructing saved transaction bytes and safely
rebroadcasting them through chain clients is the separate work in maker #648;
it does not require or add a public maker raw-broadcast route.

## BOLT12 route family

The retained Rust methods require these exact upstream routes:

| Rust method | Required HTTP route |
| --- | --- |
| `post_bolt12_offer` | `POST /v2/lightning/BTC/bolt12` |
| `patch_bolt12_offer` | `PATCH /v2/lightning/BTC/bolt12` |
| `delete_bolt12_offer` | `POST /v2/lightning/BTC/bolt12/delete` |
| `get_bolt12_invoice` | `POST /v2/lightning/BTC/bolt12/fetch` |
| `get_bolt12_params` | `GET /v2/lightning/BTC/bolt12/L-BTC` |

KaleidoSwap's operator node-offer create/pay/decode routes and existing invoice
fields do not establish support for this public Boltz route family. This policy
does not remove those other capabilities or change swap creation behavior.

## Maintaining the boundary

`make check-compatibility-boundary` checks the retained Rust route literals,
binding exports, distinct chain/submarine helpers and the pinned maker route
inventory. CI runs it alongside binding parity. Regression tests exercise new
public and parameterized routes, accidental raw exports, removed Rust support
and the formerly incorrect parity alias.

The committed `tests/fixtures/maker-compatibility-routes.json` is extracted from
the actual maker router and records its commit and source SHA-256. It does not
automatically discover future changes in the private maker repository. To check
a fresh checkout or refresh the inventory with an authenticated `gh` account
that can read the maker repository:

```bash
python3 scripts/check_compatibility_boundary.py --maker-router /path/to/maker/crates/maker-api/src/lib.rs
python3 scripts/check_compatibility_boundary.py --refresh-maker main
```

The refresh validates the policy before writing the inventory. If a previously
absent route appears, review its authorization and behavior, update this policy
and the regression expectations, then refresh. Route registration alone is not
proof that the deployed backend supports an end-to-end operation.
