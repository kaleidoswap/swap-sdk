# L-USDT PR review remediation plan

> Status: **R1–R10 implemented on both feature branches; awaiting pushed CI.**
>
> Scope: follow-up work discovered while reviewing SDK PR #6 and Maker PR
> #124 after Phase 5. This is deliberately separate from the phased feature
> plan in `docs/lusdt-swaps-plan.md`.

## Outcomes

- Restore every legacy L-BTC path affected by the L-USDT changes.
- Prevent Liquid/L-USDT swaps from entering Bitcoin-only cooperative signing.
- Preserve the caller's explicit/confidential payout intent through PSET funding.
- Keep binding behavior and errors equivalent to the Rust SDK.
- Reject funded PSETs that are internally inconsistent or non-relayable.
- Return both PRs to a green required-check state before declaring review complete.

## Merge blockers

### R1 — Legacy L-BTC refund UTXO selection

**Problem:** strict expected-amount matching is required for caller-funded L-USDT
spends, but it changed the established L-BTC refund behavior. Legacy refund tests
intentionally exercise an underpayment and spend the amount actually present in
the HTLC.

**Implementation:** make selection aware of `SwapTxKind`. Claims and all
non-policy-asset spends keep exact script, txid, asset, and amount matching.
Legacy L-BTC refunds keep exact script/txid/asset validation but accept the
positive decoded value actually locked.

**Acceptance:** all four failing Electrum/Esplora refund tests pass; adversarial
L-USDT amount-decoy tests remain rejected.

### R2 — Disable Bitcoin cooperative signing for Liquid/L-USDT swaps

**Problem:** Maker cooperative reverse-claim and submarine-refund loaders check
only swap type/state. A Liquid row can reach Bitcoin leaf/session construction,
and the endpoint can mutate state despite returning an unusable signature.

**Implementation:** classify the persisted swap's on-chain layer before any
session construction or state mutation. Reject Liquid-backed rows with a stable
`unsupported_cooperative` API error. Apply the guard in both loaders and add
tests proving state is unchanged.

**Acceptance:** BTC cooperative flows are unchanged; Liquid/L-USDT reverse claim
and submarine refund fail before signing and before state transition.

### R3 — Restore Maker regtest driver compilation

**Problem:** `LiquidSubmarineClaim` gained an `aggregation_order` field, but the
`chain_arklbtc_atomic` initializer was not updated.

**Implementation:** pass the same claim-first aggregation order used when the
corresponding HTLC key is constructed.

**Acceptance:** every e2e driver target in `Dockerfile.driver` compiles.

### R4 — Pin payout confidentiality intent

**Problem:** `PreparedLiquidSpend` currently pins only the destination script.
A wallet can replace a confidential payout with an explicit output, or attach a
different blinding key, without changing the script.

**Implementation:** store the parsed destination blinding pubkey in the prepared
intent. During finalization require the PSET output blinding metadata and the
extracted transaction's explicit/confidential form to match that intent. Verify
the wallet-supplied output secrets only for a confidential destination; require
zero secrets for an explicit destination.

**Acceptance:** explicit and confidential happy paths pass; confidentiality
downgrade, unexpected blinding, and wrong blinding-key metadata are rejected.

### R5 — Preserve typed WASM errors

**Problem:** WASM converts core errors into primitive strings, losing the stable
error identity such as `liquid_fee_asset_required`.

**Implementation:** throw a JavaScript `Error` whose `name` and `code` contain
the core error name and whose message contains the human-readable detail. Export
a TypeScript error interface and type guard.

**Acceptance:** callers can branch on `error.code` without parsing text.

## Binding and validation parity

### R6 — Type the TypeScript PSET façade

Wrap the raw generated WASM `SwapScript` and `PreparedLiquidSpend` classes in the
public TypeScript entry point. Give prepare/template/finalize methods concrete
`LiquidPset*` parameter and return types while retaining the generated classes
internally.

### R7 — Expose local lockup transactions in PSET bindings

The Rust API supports `TransactionOptions.lockup_tx`; UniFFI and WASM currently
force `None`. Add an optional binding-safe lockup transaction (object in UniFFI,
Liquid transaction hex in WASM/TypeScript), parse it as Liquid, and forward it
through `TransactionOptions`.

### R8 — Strengthen funded-input validation

PSET finalization is intentionally offline and therefore cannot prove chain
existence. Document that the caller/wallet must source confirmed inputs. When a
PSET includes `non_witness_utxo`, verify its txid, vout, and output exactly match
the input outpoint and `witness_utxo`; reject contradictory metadata. Retain
`witness_utxo` as the standard Elements signing input.

### R9 — Enforce a relay fee floor and complexity limits

After inserting the final swap witness, compute discounted virtual size and
require at least the Liquid minimum relay fee, in addition to the existing
effective maximum fee. Reject zero-fee and unreasonably large caller-funded
PSETs before expensive proof/signature work.

### R10 — Clarify template indices and API evolution

Document that template indices describe the initial template only; the wallet
may insert inputs/outputs and the finalizer re-derives the protected indices.
Mark the public Rust error enum non-exhaustive where compatible. Treat the new
asset/PSET surface as a minor release and bump package versions only in the
release commit, not in this review-fix commit.

## Commit and verification plan

1. **SDK review fixes:** R1, R4–R10; run formatting, Rust unit/integration tests,
   bindings tests, WASM build, and TypeScript checks.
2. **Maker review fixes:** R2–R3; run focused cooperative/API tests, workspace
   formatting/checks, and compile all regtest driver binaries.
3. Commit once per repository with `fix(lusdt): address PR review findings`, push
   both existing feature branches, and reply to each actionable PR comment with
   the commit and verification evidence.
4. Update SDK issue #5 with a review-remediation checklist and current CI state.
5. Re-fetch unresolved threads and required checks. Review is complete only when
   no actionable thread remains and every required check is green.
