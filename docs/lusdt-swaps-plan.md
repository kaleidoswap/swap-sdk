# Plan: Add native Liquid USDt (LUSDT) swaps alongside L-BTC

## Goal & scope

Add support for **native Liquid Tether (USDt)** — the Liquid-issued asset
`ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2` on mainnet —
as a swappable asset in the **Boltz Liquid swap engine** (`src/swaps/liquid.rs`),
alongside the existing L-BTC support.

This is explicitly **not** about RGB-USDT. All the `USDT`/`Tether` strings that
already exist in the repo belong to the RGB Lightning Node (RLN) client
(`rln-client/`, `specs/rgb-lightning-node.yaml`) and are a completely separate
mechanism (RGB assets over Lightning). Nothing in this plan touches RLN.

### Out of scope (tracked, not done here)

- Any RGB-USDT / RLN changes.
- Server-side (KaleidoSwap/Boltz backend) work. The SDK is a **client**; a
  Liquid-USDT swap pair must be offered by the backend for these swaps to
  function end to end. See "Backend dependency" below.

## Background: how the Liquid engine works today

- **`network/mod.rs`**
  - `Chain::Liquid(LiquidChain)` — the `Chain` enum carries **no asset**;
    "Liquid" implicitly means L-BTC everywhere.
  - `LiquidChain::bitcoin()` (mod.rs:68) returns the **policy (L-BTC) asset id**
    per network (mainnet `AssetId::LIQUID_BTC`, testnet/regtest hardcoded).
  - `LiquidClient` trait — esplora/electrum backends: get UTXO/tx, broadcast,
    genesis hash.
- **`LBtcSwapScript`** (liquid.rs:75) — taproot swap script (hashlock claim leaf
  + CLTV refund leaf), MuSig2 key aggregation, confidential blinded address.
- **`LBtcSwapTx`** (liquid.rs:540) — builds claim/refund txs.
  **Current shape: exactly 1 confidential input → 1 confidential payment output
  + 1 explicit fee output**, fully blinded (asset blinding factor + surjection
  proof, value blinding factor + rangeproof). `is_discount_ct = true` is passed
  for Liquid (ELIP-0200 discount vsize).

The transaction **value/asset math is already asset-agnostic**: `create_claim`
(liquid.rs:829) and `create_refund` (liquid.rs:1102) read the asset from the
unblinded funding UTXO (`unblined_utxo.asset`) and reuse it for the outputs. The
blinding logic doesn't care which asset it is.

## The core problem: Liquid fees must be paid in L-BTC

Today the fee output is `TxOut::new_fee(absolute_fees, asset_id)` (liquid.rs:893,
1185) where `asset_id` is the **funding** asset, and the fee is subtracted from
the swap amount. That is only valid because the funding asset **is** L-BTC.

For an LUSDT swap the funding UTXO is LUSDT, so:

- The fee **cannot** be denominated in or subtracted from the LUSDT amount —
  Liquid network fees are always paid in L-BTC.
- The claim/refund transaction therefore needs a **second input providing L-BTC**
  to cover the fee, plus an **L-BTC change output**.

New transaction shape for an LUSDT claim/refund:

```
inputs:  [ LUSDT swap utxo ] + [ L-BTC fee utxo ]
outputs: [ LUSDT -> user (full swap amount) ]
         [ L-BTC change -> user ]
         [ explicit L-BTC fee ]
```

This is the single biggest structural change and it breaks the documented
"reverse swap spends only 1 utxo / sweep is 1 output" assumptions in the README.
Multi-asset blinding must balance value blinding factors **per asset**
(LUSDT in vs LUSDT out; L-BTC in vs L-BTC change + fee) and produce a surjection
proof per confidential output against the full input set.

## Design decisions

1. **Thread an explicit `AssetId` through the Liquid path** rather than deriving
   "the asset" from `LiquidChain::bitcoin()`. Introduce the notion of the swap's
   *funding asset* and validate against it, instead of hardcoding L-BTC.

2. **Asset registry per network.** Add an `lusdt()` (and a generic lookup) to
   `LiquidChain` returning the LUSDT `AssetId` per network:
   - mainnet: `ce091c99…a37bd2`
   - testnet: (confirm the Liquid-testnet USDt asset id, or make configurable)
   - regtest: issued at runtime by the regtest env → must be **injectable**,
     not a constant.
   Because regtest ids are dynamic, prefer carrying the asset id on the swap
   type over a pure enum-to-constant map. Recommended: add an
   `asset_id: AssetId` field to `LBtcSwapScript` / `LBtcSwapTx` (defaulted to the
   network's L-BTC id for existing call sites) so the engine never has to guess.

3. **L-BTC fee funding is caller-supplied.** The SDK cannot source an L-BTC UTXO
   on its own. Add an optional fee-input parameter (outpoint + `TxOut` +
   blinding secret key + a change address) that callers pass when the swap asset
   is not L-BTC. When the asset **is** L-BTC, behaviour is unchanged (fee comes
   out of the swap output, single input).

4. **Currency strings stay at the caller / thin mapping layer.** `from`/`to` in
   the Boltz request structs are already `String`. Add the USDT currency string
   and pair accessors; do not over-engineer a new enum unless the backend
   contract requires it.

## File-by-file changes (ordered)

### Phase 1 — asset plumbing (no behaviour change for L-BTC)

1. **`src/network/mod.rs`**
   - Add `LiquidChain::lusdt() -> AssetId` (mainnet constant; testnet
     constant/config; regtest injectable).
   - Keep `bitcoin()` as-is (it is the policy/fee asset).
   - Consider a small helper describing "policy asset for fees" vs "swap asset".

2. **`src/swaps/liquid.rs`**
   - Add `asset_id: AssetId` to `LBtcSwapScript` (and/or `LBtcSwapTx`), populated
     from the create-swap responses / caller. Default it to
     `network.bitcoin()` so all existing constructors keep working.
   - Replace the hard check in **`unblind_utxo()` (liquid.rs:64)**
     `secrets.asset != network.bitcoin()` with a check against the **expected
     swap asset** (`!= expected_asset`), taking the expected asset as a param.

3. **`src/swaps/wrappers.rs`**
   - **`construct_claim` (wrappers.rs:647)** — pass the swap's expected asset into
     `unblind_utxo` instead of `chain_client.network()`'s L-BTC.
   - **`check_direct_transaction_inner` (wrappers.rs:514)** — replace
     `asset != chain.bitcoin()` with the expected swap asset.

### Phase 2 — fee funding (the structural change)

4. **`src/swaps/liquid.rs`**
   - Extend `LBtcSwapTx` with an optional L-BTC fee input:
     `fee_input: Option<{ outpoint, txout, blinding_key, change_address }>`.
   - Rework `create_claim` / `create_refund` to two branches:
     - **funding asset == L-BTC**: current single-input path, unchanged.
     - **funding asset != L-BTC**: build the 2-input / 3-output tx above with
       multi-asset blinding (per-asset value blinding, one surjection proof per
       confidential output over both inputs, fee output explicit L-BTC).
   - Update `size()` / `tx_size` estimation for the larger tx (still honour
     `is_discount_ct`).
   - Update `sign_claim` / `sign_refund` (and the cooperative MuSig2 paths) to
     sign **both** inputs; the fee input is a normal key-path/single-sig spend
     from the caller-provided L-BTC key (not part of the MuSig2 swap key agg).

5. **`src/swaps/fees.rs` + `src/util/fees.rs`**
   - `estimate_claim_fee` and `mrh_amount = lockup − claim_fee` (wrappers.rs:322)
     assume the fee is subtracted from the swap asset — split fee estimation
     (in L-BTC) from the swap output amount for non-L-BTC swaps. Add LUSDT tx-size
     constants (2-in/3-out) alongside `LIQUID_TX_SIZES`.

### Phase 3 — magic routing & currency surface

6. **`src/swaps/magic_routing.rs`**
   - `check_for_mrh` (lines 102-119) hardcodes `LBTC_*_ASSET_HASH`. Generalise to
     accept the expected swap asset (L-BTC **or** LUSDT) so a USDT BIP21 isn't
     rejected. (MRH for USDT only matters if the backend issues USDT MRHs;
     otherwise gate MRH to L-BTC and skip for USDT.)

7. **`src/network/mod.rs` (Display) + `src/swaps/boltz.rs`**
   - Decide the currency string the backend expects for Liquid-USDT (e.g.
     `"USDT"` / `"L-USDT"`), and:
     - Add pair accessors on `GetSubmarinePairsResponse` /
       `GetReversePairsResponse` / `GetChainPairsResponse` (boltz.rs:258-330) for
       the USDT keys.
     - Provide a way to set `from`/`to` to the USDT string (either a richer
       `Chain`/currency type or a documented caller convention — today callers do
       `chain.to_string()`).

### Phase 4 — bindings & docs

8. **Bindings**
   - `bindings-wasm/src/lib.rs`, Python UniFFI (`bindings/`), and
     `typescript-sdk/` expose the swap surface; surface the new asset/currency
     and the fee-input parameter. (The `issueAsset*`/`listAssets` methods there
     are RLN/RGB — leave untouched.)

9. **`README.md`**
   - Update the "Assumptions" section (single-utxo / single-output) to reflect
     the 2-in/3-out LUSDT case.

## Testing plan

- **Regtest** (`tests/regtest/`, requires the docker env): issue a regtest USDT
  asset, fund a swap script with it plus a separate L-BTC utxo, and exercise
  reverse-claim and submarine-refund with the new fee input. Mirror the existing
  `reverse.rs` / `submarine.rs` flows.
- **Unit** (`tests/txs.rs`, `src/swaps/liquid.rs` tests): extend
  `prepare_lbtc_claim` / `prepare_lbtc_refund` analogues for a non-L-BTC funding
  asset; assert outputs = [USDT to user, L-BTC change, explicit fee] and that
  blinding validates. Add a multi-asset `tx_size` vector.
- **Negative**: wrong fee-asset, insufficient L-BTC fee input, USDT amount
  underflow.

## Backend dependency (must confirm before Phase 3+ is testable)

The SDK can build and sign LUSDT swap transactions, but a full swap requires the
**KaleidoSwap/Boltz backend to offer a Liquid-USDT pair** (currency strings,
`GET /swap/*/pairs` entries, lockup addresses funded in USDT, cooperative MuSig2
partial-sig endpoints for the USDT swaps). Confirm:

1. Which currency string the backend uses for Liquid-USDT.
2. Whether the backend funds USDT lockups and expects USDT claim/refund txs with
   a client-supplied L-BTC fee input (vs. fee sponsorship / discount-CT covered
   by the server).
3. Whether MRH is issued for USDT.

The answers to (2) in particular could simplify or reshape Phase 2.

## Risks

- **Multi-asset blinding** is the trickiest part; getting per-asset value
  blinding factors and surjection proofs right is where bugs will hide. Lean on
  regtest end-to-end validation, not just size estimation.
- **Breaking the 1-utxo/1-output invariants** ripples into fee estimation, size
  calc, and the cooperative-claim MuSig2 path (now signing 2 inputs).
- **Regtest USDT asset id is dynamic** — the design must inject it, not hardcode.
