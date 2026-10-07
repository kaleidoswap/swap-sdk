# Plan: USDT-RGB (RGB on Bitcoin L1) submarine and reverse swaps

> Status: **Phases 1–4 implemented and validated on regtest.** Tracks the maker work in
> kaleidoswap/kaleidoswap-maker-rs#551 (cases 2a and 2b). The maker side ships
> behind `[rgb] enabled = false`, and its pairs are seeded disabled.

## Goal & scope

Let a client swap an RGB asset held on Bitcoin L1 against Lightning BTC through
the KaleidoSwap maker's standard Boltz-v2 HTLC swaps:

- **Submarine (2a):** `USDT-RGB → BTC Lightning`. The taker locks the asset in
  the swap tree, the maker pays the invoice and claims through the claim leaf,
  and the taker refunds through the refund leaf after the timeout.
- **Reverse (2b):** `BTC Lightning → USDT-RGB`. The maker locks the asset once
  the hold invoice is accepted, the taker claims through the claim leaf (which
  reveals the preimage), and the maker settles the hold.

The atomic PSBT swap (case 1, `/v2/swap/atomic`) is out of scope here. It is a
different protocol (rgb-lib offer → request → proposal → completion, no HTLC)
and gets its own plan.

This mirrors the L-USDT design in [`lusdt-swaps-plan.md`](lusdt-swaps-plan.md):

| L-USDT V1 | USDT-RGB V1 |
|---|---|
| Explicit L-USDT HTLC, plus a policy-asset fee | Boltz P2TR HTLC carrying `htlcSat` sats, plus an RGB allocation |
| Caller wallet funds the L-BTC fee through a PSET | The fee comes from `htlcSat`, or the caller wallet adds BTC inputs |
| SDK validates the funded PSET, then signs the HTLC input | SDK validates the funded **and colored** PSBT, then signs the HTLC input |
| Script-path only, no cooperative MuSig | Script-path only. The maker answers `422 unsupported_cooperative` |
| `LiquidAssetContext`, `requires_caller_funded_pset()` | `RgbHtlcContext`, `is_rgb()` guard on every uncolored spend path |

## The SDK/wallet boundary

The SDK does **not** depend on rgb-lib. rgb-lib is heavy, native-only (SQLite,
no wasm) and wallet-stateful. Every RGB state operation stays in the caller's
RGB wallet, and the SDK owns everything about the HTLC:

| Step | Who | What |
|---|---|---|
| Create, validate | SDK | Tree, keys, hashlock, timelock, address, `scriptPubkey`, `recipientId`, amount, contract id, `htlcSat` |
| 2a lock | wallet | rgb-lib `send` (donation) to the validated `RgbLockInstructions` |
| 2b accept lock | wallet | `fetch_and_accept_transfer_by_recipient_id` (or the split fetch + `accept_transfer_pinned`), then `save_new_asset` |
| Spend template | SDK | Unsigned PSBT: HTLC input, `OP_RETURN` (vout 0), colored payout (vout 1) |
| Fund (optional) | wallet | Add BTC inputs and change after vout 1, sign nothing yet |
| Color | wallet | `psbt_op_prepare_with_expiry(output_map {1: amount})` |
| Finalize | SDK | Check that coloring touched only the commitment, check the allocations, sign the HTLC leaf |
| Sign rest, broadcast | wallet | `sign_psbt`/`finalize_psbt` for its own inputs, `psbt_op_mark_broadcast`, broadcast, `psbt_op_apply`, `psbt_op_provide_receive_consignment` |

A spend whose PSBT lacks the 32-byte RGB commitment at output 0 **burns the
asset**. So the SDK makes an uncolored spend of an RGB HTLC unreachable:
`BtcSwapTx::new_claim/new_refund`, `sign_*`, `partial_sign` and
`SwapScript::construct_*` all refuse a script that carries an RGB context.

## Wire contract (maker PR #551)

No request changes: the taker sends the Boltz request with
`from: "USDT-RGB", to: "BTC"` (submarine) or `from: "BTC", to: "USDT-RGB"`
(reverse). The create responses gain an `rgb` object:

```json
"rgb": {
  "assetId": "rgb:…",            // contract id
  "amount": 1005,                // contract units; == expectedAmount / onchainAmount
  "recipientId": "bcrt:wvout:…", // rgb-lib witness recipient of scriptPubkey
  "blinding": "123…",            // decimal u64 chosen by the maker
  "htlcSat": 1000,               // sats on the HTLC output
  "claimFeeRate": 5,             // reverse only: the claim fee rate htlcSat funds
  "scriptPubkey": "5120…",       // == address / lockupAddress script
  "transportEndpoints": ["rpcs://…/json-rpc"],
  "minConfirmations": 1
}
```

Units follow each asset leg. Pair `limits` use the **input** asset's base
units; `rate` is output units per input unit; `minerFees` use the **output**
asset's base units. Thus submarine limits use RGB contract units and its fees
use BTC sats; reverse limits use BTC sats and its fees use RGB contract units.
`expectedAmount` (RGB submarine), `onchainAmount` (RGB reverse) and `rgb.amount`
use contract units (6 decimals for USDT-RGB). `htlcSat` always uses BTC sats.

The HTLC is the standard Boltz taproot tree (leaf version `0xc0`), with the
same MuSig aggregation order as BTC: submarine is claim-first, reverse is
refund-first. The SDK validates it through the existing `BtcSwapScript`
reconstruction and address check.

### Response validation (`validate_rgb`)

The response is rejected unless all of these hold:

1. The Boltz checks pass: hashlock, CLTV equal to `timeoutBlockHeight`, our
   key in the right leaf, address recomputed. Both wire leaves must exactly
   match the reconstructed scripts and use leaf version `0xc0`.
2. `rgb` is present, and `rgb.assetId` equals the contract id the **caller**
   pins. The pair cards carry no contract id, and in a reverse swap a
   substituted contract pays the taker in a worthless asset.
3. `rgb.scriptPubkey` equals the script of `address` / `lockupAddress`.
4. `rgb.recipientId` is the canonical rgb-lib encoding of that script: a
   network prefix compatible with the swap's chain, then
   `wvout:` + baid64(`05 ‖ output key`) with the embedded checksum. A recipient
   id for another script would send the taker's lock somewhere else. The SDK
   ports the ~40-line encoder, and golden vectors from `rgb-invoicing`
   0.11.1-rc.11 pin it.
5. `rgb.amount == expectedAmount` (2a) or `onchainAmount` (2b), and is
   positive.
6. `546 ≤ htlcSat ≤ 1000` on submarines by default. These BTC sats are paid
   to the maker on success; `max_fee` only caps a later claim/refund fee. A
   deliberate local override uses `validate_rgb_with_max_htlc_sat`, native
   `rgb_max_htlc_sat`, or the wasm submarine create method's fourth argument.
   Never derive the cap from the response. On a reverse swap, `claimFeeRate` is present and
   `htlcSat` funds a self-funded claim at that rate with ≥ 546 sat left over.
7. `minConfirmations ≥ 1`, and every transport endpoint is `rpc://` or
   `rpcs://`.
8. `blinding` parses as a `u64`.

A BTC (non-RGB) response that carries an `rgb` object is rejected too.

## Spend construction

`PreparedRgbSpend` mirrors `PreparedLiquidSpend`:

```text
SwapScript::prepare_rgb_claim(RgbPsbtParams)   // 2b
SwapScript::prepare_rgb_refund(RgbPsbtParams)  // 2a, after timeoutBlockHeight
PreparedRgbSpend::template() -> RgbPsbtTemplate
PreparedRgbSpend::fund(funded_psbt) -> PreparedRgbSpend    // CallerInputs only
PreparedRgbSpend::finalize_claim(ColoredRgbPsbt, keys, preimage) -> FinalizedRgbSpend
PreparedRgbSpend::finalize_refund(ColoredRgbPsbt, keys)       -> FinalizedRgbSpend
```

There are two funding modes:

- **`HtlcValue { fee_rate }`.** `[HTLC] → [OP_RETURN, payout = htlcSat − fee]`.
  This is exactly the maker's reference taker claim (195 vB for the 61-byte
  reverse claim leaf, so `htlcSat = 195·claimFeeRate + 546`). It is refused
  with the typed `rgb_fee_input_required` when less than 546 sat would be
  left.
- **`CallerInputs`.** `[HTLC, wallet inputs…] → [OP_RETURN, payout = htlcSat,
  wallet change…]`. This is the maker's own spend shape, and the way to refund
  a 2a lock whose `htlcSat` cannot cover the fee.

The template is version 2, with `nLockTime` 0 for a claim and the CLTV for a
refund. The HTLC input uses sequence `0xFFFFFFFD` and has `witness_utxo` set.

**Invariants checked at `fund`** (on the funded but uncolored tx):

- version and locktime unchanged;
- exactly one HTLC input, at the pinned outpoint, with the pinned prevout and
  sequence;
- no duplicate inputs, and `witness_utxo` on every input;
- outputs 0 and 1 unchanged, with no other `OP_RETURN`;
- `0 < fee ≤ max_fee`.

The funded tx is then frozen. `fund` returns a new immutable spend; its
`template()` returns the funded PSBT, the current HTLC input index and
`requiresFunding = false`. Keep this new spend for coloring and finalization.

**Invariants checked at `finalize`:**

- The colored tx equals the frozen tx, except that output 0's script became
  `OP_RETURN <32 bytes>`. This mirrors the maker's
  `ColoredSpend::from_prepared`.
- The allocations rgb-lib returned are exactly
  `[{assetId, vout: 1, amount}]`.
- The HTLC input is still unsigned.

Then the SDK signs the script path with `Prevouts::All` at the re-derived HTLC
input index. The witness is `[sig, preimage, claim leaf, control block]` for a
claim and `[sig, refund leaf, control block]` for a refund. The SDK sets
`final_script_witness` and returns the PSBT. When every input is final, it also
returns the extracted transaction.

## Status flow (what the client observes)

**Submarine:**
- `swap.created` / `invoice.set`.
- `transaction.mempool` and `transaction.confirmed` arrive together, only after
  the maker has verified the lock at `minConfirmations`. RGB has no 0-conf
  `mempool`.
- `invoice.pending`, then `invoice.paid`, then `transaction.claimed`.
- Failures: `invoice.failedToPay`, `swap.expired`, `transaction.failed`, plus
  `paymentStatus` `underpaid` / `wrong_asset`. These leave only the refund
  path.

**Reverse:**
- `swap.created`, then `transaction.mempool` (lock broadcast), then
  `invoice.settled`.
- There is **no** `transaction.confirmed`. The client counts the lock's
  confirmations against `minConfirmations` itself; rgb-lib's accept enforces
  it.
- The claim must confirm before `timeoutBlockHeight`, when the maker's refund
  becomes valid.
- `claimFeeRate` is a quote, not a confirmation guarantee. Before paying the
  invoice, compare it with a locally chosen minimum from current fee estimates
  and the remaining timeout. Reject an inadequate quote. If fees rise later,
  select a higher local spend rate; use `CallerInputs` when the HTLC cannot fund
  it, and monitor confirmation until settlement. The SDK does not set a static
  network fee floor.
- RGB spends require the actual accepted colored lock transaction. Never use
  address discovery to choose a colored outpoint; unrelated BTC can pay the
  same address. Contract comparison ignores only cosmetic chunk dashes; callers
  must pin an actual contract id from their trusted wallet or asset registry.

## Phases

1. **Rust core (this branch).**
   - `Currency::UsdtRgb` (`"USDT-RGB"`, Bitcoin only).
   - `rgb` on both create responses.
   - `validate_rgb`, `RgbHtlcContext`, and the recipient-id codec.
   - `PreparedRgbSpend`.
   - `SwapScript::prepare_rgb_*`.
   - Guards on every uncolored path.
   - `Error::RgbFeeInputRequired`.
   - Unit tests: codec vectors, fee model, tampering.
2. **Bindings (implemented).** UniFFI records and `PreparedRgbSpend`, regenerated
   Python glue, wasm/TypeScript facade, and `check-binding-parity`. Native create
   request records take optional `rgb_contract_id`; wasm create methods take an
   optional third `rgbContractId` argument. RGB routes require a nonempty pin
   before the POST; responses go through `validate_rgb`. RGB chain swaps remain
   unsupported. Native `BtcLikeTransaction::from_hex_bitcoin` lets wallets supply
   the lock transaction. Both bindings return the finalized PSBT and an optional
   broadcastable transaction; caller-funded spends may still need wallet signatures.
   Synthetic binding vectors exercise both directions without a live maker or RGB
   proofs. They do not replace the frozen maker wire fixture or live wallet tests.
3. **Wire fixture (implemented).** `tests/fixtures/rgb-v1/` freezes actual
   maker router requests, pair cards and both create responses at `49c6ce2`.
   The mock venue calls the pinned rgb-lib encoder. Golden vectors cover six
   networks and the maker's reference claim PSBT. `tests/rgb_contract.rs`
   validates the SDK; [maker companion PR #668](https://github.com/kaleidoswap/kaleidoswap-maker-rs/pull/668) pins its schemas, trees,
   MuSig order, recipient codec and fee model. See the fixture README for capture
   provenance and reproduction. The companion PR targets the maker RGB branch
   for inclusion in #551.
4. **Live validation (implemented).** The standalone native crate
   [`examples/rgb-regtest`](../examples/rgb-regtest/README.md) drives real
   rgb-lib wallets against the maker daemon, Bitcoin/Esplora, an RGB proxy and
   two real Lightning nodes: 2a payment/claim, 2a failed-payment refund and 2b
   claim without taker BTC. It has a separate workspace so RGB wallet/node
   dependencies stay outside the SDK and wasm graph.

## Phase 2 validation (2026-10-06)

- Native workspace Clippy (`--all-targets --all-features -D warnings`) and
  `make wasm-clippy` pass. Daemon downloads were disabled for native checks;
  no daemon-backed test is claimed here.
- `cargo test --lib rgb`: 15 tests pass, including the funded-template/index
  regression check.
- `cargo test -p bindings --test test_generated_bindings rgb`: generated Python
  create/claim/refund workflow passes with a local HTTP maker stub. The committed
  fallback glue is exercised against the same synthetic vectors.
- Python glue regenerates deterministically, verified by comparing its pre-run
  and post-run bytes. `make check-generated` checks the committed snapshot for drift.
- Binding parity, Rust/Python formatting, TypeScript lint/format/typecheck and
  example typecheck pass. The wasm package builds; all 70 TypeScript tests pass.
- The shared vectors in `bindings/tests/fixtures/rgb-spends.json` are synthetic
  Bitcoin/PSBT vectors. Phase 3 now supplies independently generated maker wire
  fixtures. RGB proofs and live wallet validation were pending at this phase; the
  Phase 4 run below now covers them.

## Phase 3 validation (2026-10-06)

- Captured both create routes and pair cards through the maker's real axum
  router against disposable PostgreSQL. Node, balances and chain backend are
  mocked; witness recipients use actual rgb-lib `96f039d` / rgb-invoicing
  `0.11.1-rc.11`. The asset was not issued and no RGB proof is claimed.
- Validation passes: 6 SDK RGB contract tests, 7 existing Liquid contract tests,
  178 SDK library tests (4 ignored), and 4 maker contract tests. Native and
  wasm Clippy, maker companion Clippy and formatting pass.
- SDK contract tests validate both responses and reject changed contracts,
  amounts, scripts, addresses, recipient networks/checksums, confirmations,
  endpoints, blinding, timelocks and claim fee budgets.
- The SDK claim matches the maker's reference unsigned PSBT and prevout; fee
  caps and insufficient BTC funding are tested. Recipient vectors cover all
  six supported RGB networks on the actual maker HTLC output keys.
- Maker companion tests reconstruct both trees and MuSig addresses, encode
  every recipient with rgb-lib, freeze response schema serialization and
  compare the reference PSBT and maker fee formulas. These run in the maker's
  existing workspace CI once the companion PR lands.
- A failing fixture mutation test exposed ignored RGB wire leaf versions/bytes.
  `validate_rgb` now requires the exact canonical tree for both directions.
- Amount documentation now distinguishes input/output asset units, including
  submarine miner fees in BTC sats and reverse miner fees in contract units.

## Phase 4 validation (2026-10-06)

- One complete run of the documented native example passed on the private
  `rgb-sdk-regtest` stack. Maker `49c6ce2` and rgb-lib `96f039d` were pinned;
  LDK server `86ca542` supplied real hold-invoice payments. An issued NIA with
  6 decimals and ticker USDT is the test asset. This is regtest validation.
- Bitcoin, Lightning, the maker daemon, PostgreSQL, RGB wallet state and RGB
  consignments are real. Only the price feed is supplied by the test client,
  at BTC/USDT 100000 with a tiny deterministic step to satisfy the maker's
  stale-quote policy. The fresh chain lacks fee estimates, so maker RGB fees
  use its configured 5 sat/vB floor.
- Submarine: response validation precedes the rgb-lib donation lock; a real
  100,000-sat invoice succeeds, the maker claim confirms, the API reaches
  `transaction.claimed`, and RGB balances settle.
- Refund: a real held payment is cancelled and reaches `invoice.failedToPay`.
  A 1000-sat HTLC at 5 sat/vB yields `RgbFeeInputRequired`; the wallet funds
  the SDK template with one BTC input, rgb-lib colors it, the SDK signs the
  refund leaf and the wallet signs its own input without changing that witness.
  Bitcoin Core rejects the early transaction with `non-final`. After maturity,
  the refund confirms and its receive consignment restores the prior RGB balance.
- Reverse: the receiver starts with zero BTC, validates the pinned contract
  before paying the real hold invoice, confirms and accepts the maker's RGB
  consignment, and colors the SDK claim. The claim has one HTLC input, pays
  975 sat from its 1521-sat value and leaves 546 sat. Its witness reveals the
  exact preimage; both the RGB receive and Lightning hold invoice settle.
- Coloring passes actual rgb-lib fascia assignments back to the SDK. Durable
  private recovery files precede payment and broadcast; wallet state, keys and
  credentials stay in ignored `run/`. The three final settled wallet balances
  sum to all 2,000,000,000 issued units (2000 test USDT).
- Independent Esplora reads confirm all three colored spends and their
  expected script-path witnesses before teardown. Example formatting, Clippy
  with `-D warnings`, shell syntax and whitespace checks pass. Live execution is opt-in; it is not added to ordinary SDK CI.
  See the [reproduction guide](../examples/rgb-regtest/README.md) and
  [sanitized run report](../examples/rgb-regtest/validation-report.json).

## Review fixes and compatibility re-confirmation (2026-10-07)

- Submarine BTC collateral is capped at 1000 sats by default, with explicit
  local overrides in Rust, UniFFI/Python and wasm. Negative, overflowing and
  mistyped wasm caps are rejected before posting. Caps are not sent to the maker.
- RGB spend parameters require the accepted colored lock transaction across
  every binding. Preparation uses only that transaction, even when an address
  UTXO list contains a third-party output. Missing locks are rejected at the
  binding boundary; a supplied transaction without the HTLC is rejected too.
- SDK validation passes: 178 library tests (4 ignored), 9 RGB contract tests,
  7 Liquid contract tests, generated and fallback Python RGB flows, and 73
  TypeScript tests. Native/wasm/example Clippy, formats, types, binding parity
  and version consistency pass. Three daemon-backed Python tests require the
  separate stopped regtest services; their connection failures do not affect
  the offline RGB checks. The earlier live run is not claimed as re-executed.
- Maker fixtures were re-confirmed at #551 head `b98883d`: all 4 contract tests,
  both target Clippy checks and formatting pass. The intervening commit only
  changes RGB admin fee caps. The tests and capture hook have moved to
  [maker PR #668](https://github.com/kaleidoswap/kaleidoswap-maker-rs/pull/668);
  the SDK patch artifact is removed. Historical capture/live-run pins remain accurate.
- The next release containing the documented breaking changes must be 0.11.0.
  This PR leaves release preparation and synchronized version updates to that
  release. Image build instructions, reverse fee/deadline policy and cosmetic
  contract-id dash normalization are documented.

## Open questions

- **2a refund funding.** With the default `htlcSat = 1000`, a self-funded
  refund only clears 546 sat at ≤ 2 sat/vB. Either the taker locks extra sats
  (which go to the maker on the happy path) or refunds with `CallerInputs`.
  The SDK supports both; the maker PR does not specify which to use.
- **rgb-lib rev.** The maker pins `kaleidoswap/rgb-lib` `96f039d` (not the
  `5c1a614` the PR body names). Caller wallets need the HTLC operation APIs:
  `psbt_op_prepare_with_expiry`, `accept_transfer_pinned` and
  `psbt_op_provide_receive_consignment`.
- **Message casing.** The maker builds rgb-lib without `camel_case`. This only
  matters for the atomic swap messages, which are out of scope here.
