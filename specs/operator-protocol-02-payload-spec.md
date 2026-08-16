# KaleidoSwap Operator Protocol — 02: Payload Spec

**Status:** Draft v0.1 · **Date:** 2026-07-21
**Depends on:** [`01-mechanism-taxonomy`](operator-protocol-01-mechanism-taxonomy.md)
**Companion (planned):** `03-transport-bindings` (HTTP + Nostr).

> The concrete request/response payloads for quoting, creating, and settling a
> swap against an operator. **Transport-agnostic** — these bodies ride over HTTP
> (`/v2/...`) or Nostr encrypted events (doc 03) unchanged.
>
> The baseline is the reference maker's existing Boltz-shaped `/v2` surface
> (`kaleidoswap-maker-rs/openapi.yaml`). This doc specifies (a) the current
> shapes verbatim where they're already right, and (b) the **additions** the
> taxonomy requires — marked **`[NEW]`**. Nothing here breaks stock-Boltz-SDK
> wire compatibility.

---

## 1. Conventions

- **Casing:** request/response fields are `camelCase` (Boltz convention). LSPS1
  bodies are `snake_case` (LSPS spec) and out of scope here.
- **Amounts:** integers in the asset's **smallest unit** (sats for BTC; asset
  precision otherwise). Never floats.
- **Keys/hashes:** hex. Pubkeys are 33-byte compressed. `preimageHash =
  SHA256(preimage)`, 32 bytes.
- **Rate-lock:** every quote returns a `hash` (`pairHash`) = SHA-256 over the
  canonical-JSON pair config. The taker passes it back on create; a mismatch is
  rejected `400 pair_hash_mismatch`. This is the price-binding mechanism.
- **Client-derived secrets:** the taker generates `preimage` and per-swap
  claim/refund keypairs locally (SDK's Rust core, BIP85). The operator never
  sees `preimage` until it is revealed on-chain/in-HTLC at claim time.

---

## 2. Flow

```
  ┌─ advertise ──┐   ┌─ quote ──┐   ┌─ create ─┐   ┌─ settle ────────┐
  │ GET pairs    │→  │ POST     │→  │ POST     │→  │ pay / lock, then │
  │ (+mechanism, │   │ /quote   │   │ /swap/*  │   │ cooperative      │
  │  atomicity)  │   │          │   │          │   │ claim/refund     │
  └──────────────┘   └──────────┘   └──────────┘   └──────────────────┘
        │                                                  │
        └──────────────── status: GET /swap/{id} + WS /v2/ws ───────────┘
```

---

## 3. Phase 1 — Advertisement (`GET /v2/swap/{type}/pairs`)

Current: `from → to → PairCard`, where `PairCard = { hash, rate, limits, fees }`,
`fees = { percentage, minerFees }`, `limits = { minimal, maximal, maximalZeroConf }`.
Pairs are already scoped per `swapType` (`submarine`/`reverse`/`chain`).

**`[NEW]` PairCard gains two fields:**

```jsonc
{
  "hash": "…", "rate": 1.0,
  "limits": { "minimal": 10000, "maximal": 25000000, "maximalZeroConf": 0 },
  "fees":   { "percentage": 0.5, "minerFees": 180 },
  "mechanism": "boltz-htlc",   // [NEW] MechanismId from doc 01 §4
  "atomicity": "atomic"        // [NEW] Tier from doc 01 §3
}
```

**`[NEW]` per-mechanism scoping.** A `(from, to, swapType)` may exist under more
than one mechanism — e.g. `BTC/L-USDT` chain as both `boltz-htlc` (`atomic`) and
`coop-send` (`trust-minimized`). The map key becomes
`from → to → mechanism → PairCard` (a backward-compatible reader that ignores the
mechanism level sees the operator's default-tier card first). The `pairId` used
downstream stays `"FROM/TO"`; mechanism is disambiguated at quote/create.

Rationale: mechanism and atomicity are **not** derivable from `swapType` (doc 01
§6). The taker cannot choose a tier without them.

---

## 4. Phase 2 — Quote (`POST /v2/quote`)

**Request** (current + `[NEW]`):

```jsonc
{
  "swapType": "chain",          // submarine | reverse | chain
  "pairId": "BTC/L-USDT",
  "fromAmount": 500000,         // smallest unit of the input asset
  "minAtomicity": "atomic",     // [NEW] optional: reject weaker-tier offers
  "mechanism": "boltz-htlc"     // [NEW] optional: pin a mechanism explicitly
}
```

- Absent `mechanism` → operator returns its best-tier offering for the pair.
- Absent `minAtomicity` → no tier floor.
- `[NEW]` If neither a mechanism nor a tier the operator can honour matches,
  respond `409 no_mechanism_for_tier` (see §9).

**Response** (current shape, unchanged):

```jsonc
{
  "pairId": "BTC/L-USDT",
  "fromAmount": 500000,
  "toAmount": 498200,           // after fees + spread
  "grossOutput": 500000,
  "fees": { "protocol": 900, "network": 180, "swap": 720, "total": 1800 },
  "rate": 0.9964,
  "hash": "…",                  // pairHash — pass back on create
  "validUntil": 1737480000      // [SEE 01 §10.5] normative: UNIX seconds
}
```

**`[NEW]` echo `mechanism` + `atomicity`** in the response so the SDK can render
the tier and the taker's `approvedQuote` (client-side slippage/expiry guard —
this is the wallet-engine L1 finding) binds to a known mechanism.

---

## 5. Phase 3 — Create

One endpoint per `swapType`. The mechanism determines which fields are
load-bearing; the taxonomy's `atomic` vs `trust-minimized` split maps onto the
already-present "atomic vs legacy plain-send" distinction in the chain request.

### 5.1 Submarine — `POST /v2/swap/submarine` (on-chain/VHTLC → LN)

Taker locks on the input layer; maker pays the taker's BOLT11 and claims the
lockup with the revealed preimage.

```jsonc
{
  "invoice": "lnbc…",           // the BOLT11 the maker pays; hash == preimageHash
  "refundPublicKey": "02…",     // taker refunds via script path if maker fails
  "pairId": "BTC/BTC",          // or from+to (stock Boltz shape)
  "preimageHash": "…",          // optional — derived from invoice when absent
  "pairHash": "…",              // rate-lock
  "webhook": null
}
```

Mechanisms: `ln-htlc` (LN leg) + `boltz-htlc`/`vhtlc` (lockup leg), per the pair.

### 5.2 Reverse — `POST /v2/swap/reverse` (LN → on-chain/VHTLC)

Maker issues a **hold invoice** on `preimageHash`; taker pays it, maker locks
on-chain, taker claims with the preimage.

```jsonc
{
  "preimageHash": "…",
  "claimPublicKey": "02…",      // taker is claim party on the maker's lockup
  "invoiceAmount": 500000,
  "pairId": "BTC/BTC",
  "pairHash": "…",
  "webhook": null
}
```

**`[NEW]` `wrapped-hodl` is a reverse-shaped create with an output the taker
cannot itself claim** (proxy Mode B, doc 01 §7). The maker holds *both* the
inbound LN HTLC and the output-leg hold. The only taker-side requirement is
`ln:pay`. Wire addition:

```jsonc
{
  "swapType": "reverse",
  "mechanism": "wrapped-hodl",  // [NEW]
  "preimageHash": "…",          // beneficiary's hash (Mode B) or taker's (Mode A)
  "invoiceAmount": 500000,
  "output": {                   // [NEW] where the delivered asset goes
    "layer": "LIQUID_LIQUID",
    "asset": "L-USDT",
    "address": "lq1…"           // or an invoice/VTXO target per output layer
  },
  "pairHash": "…"
}
```

### 5.3 Chain — `POST /v2/swap/chain` (on-chain ↔ on-chain)

The generic L1↔L1 swap. The request already encodes the atomic/plain-send split:

```jsonc
{
  "preimageHash": "…",
  "refundPublicKey": "02…",      // taker refund key, BTC side
  "claimPublicKey": "02…",       // [atomic only] taker claim key on maker's L-USDT HTLC
  "pairId": "BTC/L-USDT",
  "userLockAmount": 500000,      // XOR serverLockAmount (exactly one)
  "pairHash": "…",
  // userAddress: "…"            // legacy plain-send (coop-send) only; atomic omits it
  "webhook": null
}
```

- `mechanism: "boltz-htlc"` (`atomic`) → `claimPublicKey` required, no `userAddress`
  (payout built from `claimDetails`).
- `mechanism: "coop-send"` (`trust-minimized`) → address-driven, `claimPublicKey`
  ignored.

### 5.4 Create responses

All create endpoints return the artefacts the client needs to watch and claim
(current shapes — `SubmarineCreateResponse`, `ReverseCreateResponse`,
`ChainCreateResponse`): an `id`, and per locked side:

```jsonc
{
  "id": "swap_…",
  "lockupAddress": "…",          // P2TR (or elements) address to fund / watch
  "serverPublicKey": "02…",      // maker's key for the MuSig2 keyspend
  "timeoutBlockHeight": 850123,  // refund becomes spendable at/after this height
  "swapTree": { … },             // SwapTree: claim leaf + refund leaf tapscripts
  "claimDetails": { … }          // [atomic] enough to build the payout/claim tx
  // reverse also returns the hold `invoice`; chain returns per-side lock info
}
```

The `swapTree` (claim leaf keyed on `preimageHash`, refund leaf keyed on
`refundPublicKey` + `timeoutBlockHeight`) is what makes the client able to
claim/refund **without trusting the operator** — the SDK's Rust core verifies it
against the amounts and keys before funding.

---

## 6. Phase 4 — Settle (cooperative claim / refund)

Default path is **MuSig2 cooperative** (keyspend, cheapest); the tapscript
claim/refund leaf is the unilateral fallback after `timeoutBlockHeight`.

Cooperative endpoints (current): submarine refund, reverse claim, reverse
refund, chain claim, chain refund. Each is a two-round MuSig2 exchange; the maker
returns its half:

```jsonc
// CooperativeSignatureResponse
{ "pubNonce": "…66-byte…", "partialSignature": "…32-byte…" }
```

The SDK combines nonces, produces the final signature, and broadcasts. **Claim
responsibility by mechanism:**

| Mechanism | Who reveals `P` | Who claims what |
|---|---|---|
| `ln-htlc` / `wrapped-hodl` | taker/beneficiary, by settling the LN HTLC | maker claims the on-chain/VHTLC lockup with `P` |
| `boltz-htlc` (submarine) | maker, by paying the taker's invoice | maker claims the taker's lockup |
| `boltz-htlc` (reverse/chain) | taker, on-chain claim | taker claims the maker's lockup, revealing `P` to the maker |
| `vhtlc` | same as boltz, on Arkade | VHTLC claim/refund leaves |
| `coop-send` | n/a (no hash-lock) | cooperative claim + payout-retry; refund is cooperative or timeout |

**Refund** is always available to the funding party after `timeoutBlockHeight`
via the refund leaf, independent of operator cooperation — this is what backs the
`atomic` tier guarantee.

---

## 7. Status (`GET /v2/swap/{id}` + `WS /v2/ws`)

`SwapStatus` mirrors the maker's `SwapState` machine (doc 01 §6):

```
created → awaiting_payment → payment_detected → payment_confirmed
        → executing → settled
                    ↘ pending_rate_decision (re-quote)
                    ↘ refunding → refunded
        ↘ expired    ↘ failed { reason }
```

- **WS** pushes each transition as it happens (the client is already subscribed);
  **GET** is the poll fallback and the resume-after-restart read (critical for the
  MV3 service-worker host — see the SDK's `SwapStore`/`resumePendingSwaps`).
- `[NEW]` a `re-quote` (`pending_rate_decision`) carries the new quote; the taker
  responds via `POST /v2/swap/{id}/rate-decision { "accept": bool }`. `false` →
  refund. This is the only place the operator can move the price after create,
  and it is always taker-gated.

---

## 8. `[NEW]` Multi-hop / any2any (open design)

Server-side, the reference maker already settles a **two-maker shared-`H`**
route (doc 01 §5). The client-facing shape is undefined; the two candidates:

- **8a — SDK-orchestrated (preferred for v1).** The SDK plans the two legs and
  drives two independent `create` calls (leg 1 against operator A, leg 2 against
  operator B) binding both to the same `preimageHash`. Operators need no
  multi-hop awareness; the SDK owns atomicity by ordering claims correctly
  (claim the leg that pays *the taker* last). Degrades to `sequenced` tier if the
  two operators' timeouts don't leave a safe claim window.
- **8b — operator-routed.** A single `create` to a routing operator that
  sub-contracts the far leg. Simpler client, but requires an operator-to-operator
  protocol and concentrates trust. Deferred.

Doc 03 (transport) and a future `04-routing` will pin this; v1 ships **8a** with
a single operator (no hop) and the two-operator case behind a flag.

---

## 9. `[NEW]` Capability negotiation

The taker MAY send its capability set (doc 01 §8) at quote time:

```jsonc
{ "swapType": "chain", "pairId": "BTC/L-USDT", "fromAmount": 500000,
  "takerCapabilities": ["ln:pay"] }     // [NEW]
```

- Present → the operator returns **only mechanisms the taker can execute**
  (e.g. an `ln:pay`-only taker gets `wrapped-hodl`, never `boltz-htlc`).
- Absent → operator returns its full offering and the **SDK filters client-side**
  from the mechanism catalogue.

Both are valid; the SDK defaults to sending capabilities (fewer round-trips, no
client-side catalogue drift).

---

## 10. Errors & versioning

Current/`[NEW]` error codes (HTTP status + stable string; over Nostr, doc 03 maps
these into the reply body):

| Code | When |
|---|---|
| `400 pair_hash_mismatch` | quoted `pairHash` ≠ current rate card |
| `409 insufficient_inventory` | maker lacks liquidity for the amount |
| `409 no_mechanism_for_tier` **`[NEW]`** | no offering meets `minAtomicity`/`mechanism` |
| `410 quote_expired` | create after `validUntil` |
| `422 unsupported_capability` **`[NEW]`** | requested mechanism needs a capability the taker didn't declare |

**Versioning:** additive only. New mechanisms, tiers, and capabilities are new
enum values (doc 01 §9); new fields are optional. An SDK newer than an operator
simply sees fewer mechanisms; an operator newer than an SDK returns fields the
SDK ignores. `GET /v2/version` + `GET /v2/info` expose the operator's supported
mechanism/capability sets for pre-flight.
