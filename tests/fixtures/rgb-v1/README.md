# RGB HTLC wire contract, version 1

These fixtures are independent of the SDK's implementation and its synthetic
binding vectors. `wire-contract.json` was captured from the maker's real axum
router (`GET /v2/swap/{submarine,reverse}` and both POST create routes), using an
isolated PostgreSQL instance and the maker's test node, chain and wallet.
`MockRgbHtlc` was configured to call the actual rgb-lib recipient encoder before
the response was produced. No response fields were replaced after capture.

Pinned sources:

- Maker PR [#551](https://github.com/kaleidoswap/kaleidoswap-maker-rs/pull/551),
  revision `49c6ce2e9554eea0d819fd3bbd267af6db45b1c0`.
- rgb-lib revision `96f039d975cf2a83712c3c0a90e703621d445425`.
- rgb-invoicing `0.11.1-rc.11`, as resolved by the maker's lockfile.

The valid contract id is a test pin; the asset was **not issued**. The invoices,
swap-auth tokens, preimage and blinding values belong only to the disposable test
state. There are no wallet secrets, consignments, RGB proofs or live funds here.
The invoice timestamps, swap ids and blinding values vary on recapture. Freeze
one capture in both repositories; tests never regenerate it automatically.

`rgb-golden-vectors.json` includes recipient round trips on all six RGB
networks, for both actual maker HTLC scripts; a reference unsigned claim PSBT
produced by `maker-layer-rgb::build_taker_claim_psbt`; and the fee model.
The claim prevout is synthetic (`11…11:0`) and its destination is the generator
point's P2TR script. The PSBT has no RGB commitment or proof. The SDK compares
its unsigned transaction and prevout with this independently produced PSBT.

The identity quote card uses 8-decimal accounting while the test contract uses
6 decimals. The 100,000-sat invoice needs 1,005 RGB units in the submarine
(470-sat miner cost, rounded up) and pays 969 RGB units in reverse (1,521-sat
HTLC plus 1,560-sat lock fee, rounded down). Pair limits use input asset units;
miner fees use output asset units. The reverse card rounds its 3,081-sat cost
up to 31 RGB units; the submarine card reports 470 BTC sats.

## Run the SDK check

```sh
BITCOIND_SKIP_DOWNLOAD=1 ELEMENTSD_SKIP_DOWNLOAD=1 cargo test --test rgb_contract
```

## Run the maker companion

The four regression tests, ignored router capture and test-mock encoder hook
now live in [maker PR #668](https://github.com/kaleidoswap/kaleidoswap-maker-rs/pull/668),
targeting the RGB branch for inclusion in #551. The SDK retains only the shared
JSON fixtures; the cross-repository patch artifact has been removed.

Compatibility was re-confirmed on 2026-10-07 against #551's current head,
`b98883d553635e352c0affa8a3815e03a911ce80`: all four maker tests pass and both JSON
files are byte-identical. The one commit after the original capture changes
admin wallet fee caps, not the swap wire contract. Capture provenance and the
live-run report retain their original revision rather than claiming a new run.

From the companion branch:

```sh
cargo test -p maker-api --test rgb_wire_contract --locked
```

The normal maker contract tests need no database or wallet initialization. Its
existing `cargo test --workspace --all-features --locked` CI includes them.

To intentionally recapture, use a **disposable** PostgreSQL server (the maker
helper creates fresh databases), then copy both JSON files into the SDK too:

```sh
TEST_DATABASE_URL=postgres://postgres:rgb-wire-test@127.0.0.1:15433/postgres \
RGB_WIRE_FIXTURE_OUTPUT="$PWD/tests/fixtures/rgb-v1" \
cargo test -p maker-api --test rgb_wire_capture -- --ignored --nocapture
```

Verify the companion and SDK tests again after any recapture. The separate
[native regtest example](../../../examples/rgb-regtest/README.md) validates real
wallets, issued assets, RGB coloring/consignments and spends; its
[sanitized run report](../../../examples/rgb-regtest/validation-report.json)
records a successful Phase 4 run.
