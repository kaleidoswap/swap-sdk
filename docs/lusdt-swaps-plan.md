# Plan: Add native Liquid USDt (L-USDT) swaps alongside L-BTC

> Status: **frozen V1 contract**. These decisions are the shared SDK ⇄ maker
> contract. Confidential maker HTLCs and cooperative MuSig spends are explicit
> follow-up work, not part of V1.

## Goal & scope

Add support for **native Liquid Tether (USDt)** as a swappable asset in the
Boltz-v2 Liquid swap engine, alongside the existing L-BTC support, using:

- **Explicit (unconfidential) L-USDT HTLC outputs.**
- **Caller-funded L-BTC network fees** via a **two-stage PSET** boundary.
- **Unilateral tapscript (script-path) claim/refund** for V1 (no cooperative
  MuSig for L-USDT).

This fits both repositories without exposing the maker wallet or redesigning the
Boltz v2 protocol.

> Not RGB-USDT. All existing `USDT`/`Tether` strings in this repo belong to the
> RGB Lightning Node (RLN) client (`rln-client/`, `specs/rgb-lightning-node.yaml`)
> — a separate mechanism. Nothing here touches RLN.

### Supported swap directions (V1)

- **Submarine:** `L-USDT → BTC Lightning`
- **Reverse:** `BTC Lightning → L-USDT`
- **Atomic chain:** `BTC on-chain → L-USDT`
- Existing BTC / L-BTC behavior remains **unchanged**.

```mermaid
flowchart LR
    A["Maker returns explicit L-USDT HTLC"] --> B["SDK validates tree, keys, address, asset and amount"]
    B --> C["SDK prepares PSET with swap input and full L-USDT payout"]
    C --> D["Caller wallet adds L-BTC fee input, change, blinds and signs"]
    D --> E["SDK validates immutable intent and signs HTLC input"]
    E --> F["Broadcast and track through standard Boltz statuses"]
```

## 1. Frozen protocol decisions (shared SDK/maker contract)

| Concern | V1 decision |
|---|---|
| Maker HTLC output | Explicit / unconfidential L-USDT |
| Payout output | May be confidential |
| Elements transaction fee | Always policy asset (normally L-BTC) |
| Fee payer for SDK claim/refund | Caller wallet |
| Transaction interchange | Base64 PSET |
| Cooperative claim/refund | **Disabled** for L-USDT |
| Spend path | Tapscript claim/refund leaves |
| API shape | Boltz v2 with optional asset extensions |
| Internal maker pair IDs | Private implementation detail |

## Repository split

- **Maker** (`kaleidoswap-maker-rs`): §2, §4, §5, §8, §9, §10, §11.
- **SDK** (`kaleidoswap-sdk`, this repo): §3, §6, §7, plus binding regeneration.
- Both: §2 golden vectors, wire JSON contract (§4).

SDK and maker work can proceed in parallel once the wire JSON and golden vectors
are frozen.

---

## 2. Maker: fix script/address compatibility first

The maker currently creates some Liquid reverse and atomic-chain addresses with
the submarine tree and the wrong aggregate-key order; the SDK will correctly
reject those addresses even with otherwise-correct JSON.

| Swap side | Script tree | MuSig aggregation order |
|---|---|---|
| Submarine / deposit | `submarine_swap_tree` | `ClaimFirst` |
| Reverse / server lock | `reverse_swap_tree` | `RefundFirst` |
| Chain user lock | Same as submarine | `ClaimFirst` |
| Chain server lock | Same as reverse | `RefundFirst` |

- Add `maker_swap::create_chain_lockup`, backed by `reverse_swap_tree`.
- Reverse Liquid creation (`reverse.rs`): `submarine_swap_tree` → `reverse_swap_tree`.
- Atomic server lock (`chain.rs`): reverse-shaped tree.
- Thread an `AggOrder` field through address construction, claim/refund params,
  provider calls and witness construction (currently hardcoded in `claim.rs`).
- Use `CooperativeKey::from_bytes_reverse` for reverse/server-lock address
  validation.
- Keep the lockup blinder absent (explicit-output architecture in `address.rs`).

**Cross-repo golden vectors** (blocking, before further work): swap tree JSON,
claim/refund leaf scripts, aggregate internal key, taproot output key, explicit
Elements address. Both repos must derive identical values from the same keys,
preimage hash and timeout.

---

## 3. SDK: separate currency from network

Core design bug: `Chain::Display` (network/mod.rs:20-27) serializes **every**
Liquid network as `"L-BTC"`. A chain identifies the *network*; it cannot also
identify the *asset*.

Add:

```rust
pub enum Currency { Btc, LBtc, LUsdt }
```

Keep `Chain` for network selection. In the UniFFI request records add:

```rust
#[uniffi(default = None)] pub from_currency: Option<Currency>;
#[uniffi(default = None)] pub to_currency: Option<Currency>;
```

Serialization rules:

- `Chain::Bitcoin(_)` permits only `Currency::Btc`.
- `Chain::Liquid(_)` permits `Currency::LBtc` or `Currency::LUsdt`.
- Absent currency preserves current behavior: Bitcoin → `BTC`, Liquid → `L-BTC`.

The raw Boltz request structs keep using strings; only the binding→core
conversion decides which string is sent. L-USDT is additive; old clients are
preserved.

In `boltz.rs` (from line 259):

- Add a defaulted `L-USDT` outer map to submarine pair responses.
- Add `get_lusdt_to_btc_pair()`.
- Add `get_btc_to_lusdt_pair()` to reverse and chain responses.
- Add optional `from_asset_id`, `to_asset_id`, `fee_asset_id` to every pair card.

---

## 4. Boltz v2 wire contract (shared)

Public wire currencies only — never internal pair IDs.

| Swap kind | Internal maker pair | Public route |
|---|---|---|
| Submarine | `L-USDT/BTC@LN` | `L-USDT/BTC` |
| Reverse | `BTC@LN/L-USDT` | `BTC/L-USDT` |
| Chain | `BTC/L-USDT-ATOMIC` | `BTC/L-USDT` |
| Legacy plain send | `BTC/L-USDT` | Hidden from SDK |

Pair cards include optional `fromAssetId`, `toAssetId`, `feeAssetId`. Existing
fee structures must match the SDK exactly:

- Submarine: `minerFees: number`
- Reverse: `minerFees: { lockup, claim }`
- Chain: `minerFees: { server, user: { lockup, claim } }`

Create-response additions:

- Submarine/reverse: optional `assetId`, `feeAssetId`.
- Each `ChainSwapDetails`: optional `assetId`, `feeAssetId`.
- Explicit Liquid lockups omit `blindingKey` or return `null`.
- Keep required compatibility fields (e.g. `bip21`); empty string acceptable for
  explicit Liquid lockups.
- Atomic chain: stop requiring `userAddress` at creation — the payout address
  belongs to transaction construction, not swap creation.

---

## 5. Maker: inverse quoting for L-USDT submarine

The SDK submarine request does not send `fromAmount`; the maker requires it. Add
`quote_for_output`:

1. Decode invoice → BTC amount.
2. Search within the pair's input limits.
3. Reuse the existing forward quote during search.
4. Return the smallest L-USDT input whose quoted output ≥ invoice amount.
5. Monotone binary search, capped at 64 iterations.
6. Reject when no in-limit amount satisfies the invoice.

Accept legacy `fromAmount` temporarily but require it to equal the canonical
inverse-quoted amount (prevents conflicting caller amounts).

---

## 6. SDK: generalize Liquid scripts & explicit outputs

In `liquid.rs` (from line 75):

- Rename `LBtcSwapScript` → `LiquidSwapScript`, `LBtcSwapTx` → `LiquidSwapTx`;
  keep deprecated type aliases so current Rust users aren't broken.
- Replace mandatory `ZKKeyPair` blinding key with `Option<ZKKeyPair>`.

Address validation (explicit rules):

- Confidential address + matching blinding key → accept.
- Explicit address + no blinding key → accept.
- Confidential address + no key → reject.
- Explicit address + supplied key → reject.
- Any reconstructed-address mismatch → reject.

Introduce:

```rust
pub struct LiquidAssetContext {
    pub swap_asset: AssetId,
    pub policy_asset: AssetId,
}
```

Resolution:

- L-USDT responses must provide both `assetId` and `feeAssetId`.
- Existing L-BTC/Boltz responses without extensions default both to
  `LiquidChain::bitcoin()`.
- Every located HTLC output must match `swap_asset` and the expected amount.

Replace `LiquidClient::get_address_utxo` with plural `get_address_utxos`. Output
discovery must select by exact script pubkey **and** expected asset **and**
expected amount **and** expected txid when supplied — never the first output at
the address.

---

## 7. SDK: caller-funded PSET boundary

The current SDK builds a single-input tx and deducts the fee from that input's
asset — invalid for L-USDT (the Elements fee output must use the policy asset).

Wallet-neutral binding records:

```rust
pub struct LiquidPsetTemplate {
    pub pset: String,
    pub swap_input_index: u32,
    pub payment_output_index: u32,
    pub swap_asset_id: String,
    pub policy_asset_id: String,
    pub amount: u64,
}
pub struct LiquidOutputSecrets {
    pub asset_id: String,
    pub value: u64,
    pub asset_blinding_factor: String,
    pub value_blinding_factor: String,
}
pub struct FundedLiquidPset {
    pub pset: String,
    pub payment_output_secrets: LiquidOutputSecrets,
    pub max_fee: u64,
}
```

Immutable `PreparedLiquidSpend`:

```text
prepare_liquid_claim(...)
prepare_liquid_refund(...)
PreparedLiquidSpend.finalize_claim(funded_pset, keypair, preimage)
PreparedLiquidSpend.finalize_refund(funded_pset, keypair)
```

Flow:

1. SDK creates a template with the HTLC input and a full-value L-USDT payout.
2. Caller wallet adds one or more L-BTC inputs.
3. Wallet adds L-BTC change and an explicit L-BTC fee output.
4. Wallet blinds outputs as needed and signs only its own inputs.
5. Wallet returns the PSET plus secrets for the designated payout output.
6. SDK validates the complete transaction.
7. SDK signs the HTLC input.
8. SDK finalizes and returns the tx for broadcast.

**Immutable-intent invariants (reject unless all hold):**

- Every input has `witness_utxo`.
- The swap input uses the expected outpoint and exact previous output.
- No duplicate inputs, peg-ins or issuance.
- The HTLC input has the expected asset and value.
- The designated payout output pays the entire HTLC L-USDT amount to the
  requested script.
- Supplied output secrets recreate the payout commitments.
- The Elements fee output is explicit and uses the policy asset.
- Fee ≤ caller cap and quoted cap.
- No L-USDT amount deducted for the network fee.
- Refund locktime/sequence satisfy the swap timeout.
- No signature-covered field changed after preparation.

Taproot sighash must use the real `swap_input_index` and `Prevouts::All` over
**every** actual previous output — remove the current input-zero / single-prevout
assumption (liquid.rs:910, 992).

Return a typed `liquid_fee_asset_required` error when the wallet can't add enough
L-BTC. Keep the old single-input path for ordinary L-BTC swaps; **L-USDT always
takes the PSET path.**

### SDK reconciliation notes (carried over from prior analysis)

These L-BTC-hardcoded paths must be explicitly scoped to L-BTC so an L-USDT flow
never trips them:

- `unblind_utxo()` (liquid.rs:64) — `asset != network.bitcoin()`; take the
  expected swap asset instead.
- `check_direct_transaction_inner` (wrappers.rs:514) — `asset != chain.bitcoin()`;
  gate to L-BTC / MRH path only.
- Magic routing (magic_routing.rs:16-19, 102-119) — hardcoded L-BTC asset
  hashes; MRH stays L-BTC-only in V1, so ensure L-USDT never reaches this
  rejection.
- README "Assumptions" section documents the 1-utxo/1-output invariant the PSET
  path breaks — update it.

---

## 8. Maker: transaction lookup endpoints

The SDK already calls these (liquid.rs:487-510); the maker must expose them:

```text
GET /v2/swap/submarine/{id}/transaction
GET /v2/swap/reverse/{id}/transaction
GET /v2/swap/chain/{id}/transactions
```

Response models must match existing SDK models (`{ id, hex, timeoutBlockHeight }`;
chain: `{ userLock, serverLock }` with nested `transaction`/`timeout`). Return
`404 transaction_not_found` before the tx exists. Add `raw_transaction(txid)` to
the Liquid provider trait (and Bitcoin analogue). SDK still validates returned
txid, HTLC script, asset and amount.

---

## 9. Maker: fix persisted statuses

Atomic chain broadcasts the server lock without the SDK-required
`transaction.server.mempool` / `transaction.server.confirmed` progression.

- Store `wire_status` on the swap row; `load_status` returns it verbatim.
- `SwapUpdate` carries the exact wire-status string; WS/webhook use it directly.
- Persist server-lock txid + `transaction.server.mempool` atomically after
  broadcast; move to `...confirmed` only after real provider confirmation.

Expected atomic sequence:

```text
swap.created → transaction.mempool → transaction.confirmed
→ transaction.server.mempool → transaction.server.confirmed → transaction.claimed
```

---

## 10. Maker: routing & immutable swap snapshots

Three migrations after `0028`:

- **`0029_sdk_pair_routes.sql`**: `wire_from`, `wire_to`, `sdk_visible`,
  `sdk_default` on `pairs`; check that `sdk_default ⇒ sdk_visible` + non-null wire
  currencies; partial unique index over `(swap_kind, wire_from, wire_to)` where
  `sdk_default`; repo methods `resolve_sdk_pair(kind, from, to)`,
  `list_sdk_pairs(kind)`.
- **`0030_swap_route_snapshots.sql`**: `from_asset/from_layer`, `to_asset/to_layer`,
  `settlement_mode`, `liquid_network`, `liquid_genesis_hash`,
  `liquid_deposit_asset`, `liquid_server_asset`, `liquid_fee_asset`,
  `user_lockup_txid`. All route/asset/timeout/reservation fields inserted in the
  same tx as the swap row. Workers/sweepers/restarts use snapshots, **not** live
  pair config or a global payout-asset fallback.
- **`0031_liquid_asset_registry.sql`**: network/genesis/policy/L-USDT bindings.
  Startup inserts an absent binding and fails on conflict with config.

Build every configured pair at startup; remove `ATOMIC_CHAIN_ENABLED` — DB
`enabled` + `sdk_default` is the sole activation. Don't make the atomic pair
public/default until full SDK e2e passes.

---

## 11. Maker: correct Liquid configuration

```rust
pub struct ResolvedLiquidConfig {
    pub network: LiquidNetwork,
    pub esplora_url: String,
    pub mnemonic: String,
    pub genesis_hash: BlockHash,
    pub policy_asset: AssetId,
    pub lusdt_asset: AssetId,
}
```

- Mainnet may use canonical defaults.
- Liquid testnet policy asset `144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49`.
- Testnet requires an explicit L-USDT asset id.
- Regtest requires genesis hash + policy + L-USDT from config.
- Never silently use mainnet L-USDT on testnet/regtest.
- Verify configured genesis hash against the connected provider before readiness.
- Keep `payout_asset_hex` as a deprecated alias for one release, then remove.

---

## 12. Implementation / merge order

1. Maker tree, aggregation-order and cross-repo golden-vector fixes.
2. Maker Liquid config, asset registry, routing metadata, swap snapshots.
3. Maker inverse quotes, standard create responses, transaction GETs, status
   persistence.
4. SDK currency separation, pair parsing, asset context, explicit HTLC support.
5. SDK PSET prepare/finalize + regenerated UniFFI/Python/TypeScript/WASM bindings.
6. Standalone SDK-driven regtest e2e, then DB activation of the atomic public
   route.

SDK and maker work parallelize after the wire JSON and golden vectors freeze.

## Acceptance gate

Complete only when a standalone test app (pinned SDK revision, **no**
maker-internal crates) passes:

- L-USDT submarine creation, maker claim, client timeout refund.
- L-USDT reverse creation, client script-path claim.
- BTC→L-USDT atomic claim and both timeout/refund paths.
- Exact WebSocket status progression.
- All three transaction lookup endpoints.
- Restart after creation, user funding and server-lock broadcast.
- Rejection of wrong asset, wrong tree, wrong key order, decoy outputs.
- Rejection of modified PSETs, excessive fees, invalid CLTV.
- Typed failure when the caller has no L-BTC fee balance.
- Existing BTC/L-BTC test suites unchanged.
