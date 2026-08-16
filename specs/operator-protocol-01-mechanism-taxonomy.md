# KaleidoSwap Operator Protocol — 01: Swap Mechanism Taxonomy

**Status:** Draft v0.1 · **Date:** 2026-07-21
**Layer:** Normative for the client SDK planner, operator pair advertisement, and maker implementations.
**Companion docs (planned):** `02-payload-spec` (quote/create/status wire types), `03-transport-bindings` (HTTP + Nostr).

> This document defines *what kinds of swaps exist*, how they compose into
> any-to-any routes, and the **atomicity guarantee** each carries. It is the
> conceptual contract the SDK's planner, an operator's pair catalogue, and the
> maker's execution engine all pin to. Wire shapes and transports are deferred
> to docs 02/03.
>
> Grounded in the current reference maker, `kaleidoswap-maker-rs`
> (`maker-core::{swap,state,layer}`, `maker-swap::plan`), and its
> `docs/wrapped-hold-invoice-proxy.md`.

---

## 1. Purpose

An operator (market maker) quotes and settles swaps. The set of swaps it can
settle is not one thing — it is a matrix of **mechanisms**, each with different:

- the **layers** it bridges (LN, Liquid, Arkade, on-chain BTC, and v2: RGB-L1/RGB-LN),
- the **lock construction** used on each leg,
- what the **taker's wallet must be able to do** to participate, and
- the **atomicity guarantee** the taker gets if something fails mid-swap.

The taxonomy makes that matrix explicit and typed, so that:

1. The **SDK planner** can resolve `(from, to, amount)` → a concrete mechanism
   (or a chain of them) filtered by what the host wallet can actually do, and
   surface the atomicity tier to the user *before* they commit funds.
2. An **operator advertises** not just pairs but `(pair, mechanism, atomicity)`
   tuples, so a taker can demand e.g. "atomic-only" and have it honoured.
3. Adding a mechanism (a new layer, wrapped-HODL, RGB) is a **new enum value**,
   never a protocol revision.

---

## 2. The core primitive: one preimage, N hash-locked legs

Every composable KaleidoSwap swap reduces to a single primitive:

> A preimage `P`, its hash `H = SHA256(P)`, and a set of **legs**, where each leg
> locks value on its layer against the *same* `H` using whatever hash-lock
> construction that layer supports. Revealing `P` to claim any one leg exposes
> `P` to the counterparty, who uses it to claim the others. No cheating window:
> a single hash secures every leg.

This is the Boltz model generalized. `maker-core::SwapLeg` already encodes a leg
as `{asset_id, layer, amount}`, and a `SwapOrder` holds exactly two of them
today; the wrapped-hold-invoice proxy design extends the same hash to a third
party (see §7). A route with more than two legs (any2any, §5) chains multiple
two-leg swaps that share `H` where the operators support it, or sequences
independent swaps where they do not.

A **second family** of executions is *not* hash-locked and does not compose via
`P`. These are terminal, single-hop, and carry weaker or different guarantees
(single-tx atomicity, AMM execution, cooperative trust-minimized). They are
first-class mechanisms in the taxonomy — the planner must reason about them —
but they can only ever be the *whole* route, never a composable leg.

---

## 3. Atomicity tiers (normative)

Every quote carries exactly one tier. The planner MUST surface it; a taker MAY
constrain routing to a minimum tier.

| Tier | Guarantee | Failure mode |
|---|---|---|
| **`atomic`** | Hash-locked on every leg with a shared `H`. Either the taker gets the output and the maker gets the input, or both refund after timeout. No state where one side is paid and the other is not. | Timeout → both sides refund via the tapscript/VHTLC refund leaf. |
| **`single-tx`** | Both asset movements settle in **one transaction** (co-signed PSBT). Atomic by construction of the tx, but not preimage-composable — cannot be a leg in a larger route. | Tx never confirms → nothing moved. |
| **`trust-minimized`** | Legs are linked but one side uses a **cooperative claim/refund + payout-retry** rather than a hash-lock. The maker cannot steal, but a stuck cooperative path may need an operator-assisted or delayed refund. | Requires cooperative refund or timeout sweep; funds recoverable but not instantly. |
| **`amm`** | Execution against a pool inside a single operator/venue trust domain (e.g. Flashnet on Spark). Instant, pool-priced, no HTLC. | Inherits the venue's settlement model; no cross-layer atomicity. |

> `atomic` is the default target. The reference maker's fully-atomic `BTC↔L-USDT`
> chain pair is currently gated off (`enabled=false`) pending live e2e; its
> shipped `BTC↔L-USDT` chain pair is `trust-minimized` (plain-send + cooperative
> claim/refund). The taxonomy names both so the SDK can advertise the real tier.

---

## 4. Mechanism catalog (normative)

Each mechanism has a stable `id`. `composable` = can be a leg in a shared-`H`
route (§5). `takerRequires` = the capability the host wallet must expose to the
SDK (§8).

| `id` | Layer(s) | Lock construction | Composable | Atomicity | `takerRequires` | maker-rs status |
|---|---|---|---|---|---|---|
| `ln-htlc` | LN ↔ (paired chain) | Native BOLT11 HTLC | ✅ | `atomic` | `ln:pay` or `ln:receive` | ✅ submarine/reverse |
| `boltz-htlc` | BTC-L1, Liquid (L-BTC, L-USDT) | P2TR tapscript HTLC + MuSig2 cooperative claim (tapscript claim/refund fallback) | ✅ | `atomic` | `onchain:btc` / `onchain:liquid` + claim/refund | ✅ submarine/reverse/chain |
| `vhtlc` | Arkade (BTC@ARK) | SHA256-VHTLC (Ark virtual HTLC) | ✅ | `atomic` | `arkade` | ✅ submarine/reverse |
| `wrapped-hodl` | any LN-payable input → any maker-deliverable output | Maker wraps the output-leg claim inside a BOLT11 **hold invoice** on `H`; settles the hold when the output leg's `P` is revealed | ✅ (as the LN leg) | `atomic` | **`ln:pay` only** | ✅ primitive shipped (`receive_for_hash`/`claim_for_hash`); route wiring per proxy doc |
| `coop-send` | BTC-L1 ↔ Liquid (L-USDT) | Plain send + cooperative claim/refund + payout-retry | ❌ | `trust-minimized` | `onchain:*` | ✅ shipped (the un-gated chain pair) |
| `rgb-ln-htlc` | RGB assets over LN (RGB-LN channels) | RGB asset rides the LN HTLC (RLN whitepaper swap) | ✅ | `atomic` | `rgb-ln` (RLN channel) | ⛔ v2 (`RlnNodeBackend` reserved) |
| `rgb-l1-psbt` | RGB-L1 ↔ BTC-L1 | BTC + RGB assignment co-signed in one PSBT | ❌ | `single-tx` | `rgb-l1` | ⛔ v2 |
| `flashnet-amm` | Spark tokens | AMM pool execution in Spark | ❌ | `spark` | `spark` | ⛔ not in maker-rs (client-side venue) |

**Notes:**

- **`wrapped-hodl` is the strategic on-ramp.** Its taker requirement is *only*
  `ln:pay` — a wallet with nothing but a standard Lightning wallet can buy/sell
  L-USDT, Arkade BTC, or (v2) RGB assets, because the maker hides every
  non-LN leg behind a hold invoice. It is the lowest integration bar and the
  default mechanism the SDK should offer to LN-only hosts. It is still `atomic`:
  the hold invoice is only settled once the output-leg preimage is revealed.
- **`ln-htlc` vs `wrapped-hodl`**: `ln-htlc` is the LN leg of a swap the taker
  settles *directly* (the taker's own node holds the HTLC); `wrapped-hodl` is
  the LN leg the *maker* holds on the taker's behalf so the taker only pays a
  normal-looking invoice. Same lock, different who-holds-it.
- **`rgb-ln-htlc` is the premium asset-trading venue**, not a fallback — RLN
  in-channel swaps are instant and truly atomic. When both sides have RGB-LN
  channels the planner should prefer it over `wrapped-hodl`.
- **`flashnet-amm` lives client-side** (via `@flashnet/sdk`), not in maker-rs. It
  is the Spark in-venue path; cross-layer Spark swaps route through
  `ln-htlc`/`wrapped-hodl` via Spark's LN interop.

---

## 5. Swap shapes: single-leg, and any-to-any composition

A **route** is what the planner produces for `(from, to, amount)`:

- **Direct** — one mechanism whose `(from, to)` the operator advertises. E.g.
  `BTC@LN → L-USDT` via `boltz-htlc` chain swap.
- **LN-bridged (any2any)** — when no direct pair exists, bridge through LN as the
  hub asset: leg 1 `from → BTC@LN`, leg 2 `BTC@LN → to`. Only `composable`
  mechanisms can be legs. Two forms:
  - **Shared-`H` multi-hop** — both legs lock on the same `H`, atomic end-to-end.
    The reference maker already implements the **two-maker** case
    (e.g. `L-USDT ↔ BTC@ARK` routed across two operators sharing a preimage) —
    this is the P2P liquidity-composition headline.
  - **Sequenced** — two independent swaps run back-to-back when operators can't
    share `H`. Atomicity degrades to per-leg; the planner MUST surface this
    (effective tier = weakest leg, and the intermediate LN balance is briefly
    at risk between legs).

**Planner contract:** resolve candidate routes; for each, compute `atomicity =
min-tier over legs`; filter by taker capabilities (§8) and any taker-imposed
minimum tier; rank by (tier, then price). The planner MUST NOT silently pick a
weaker-tier route than requested.

> Bridge-asset choice is LN today because submarine swaps natively give
> `chain ↔ LN` pairs, keeping the pair matrix O(n) instead of O(n²). This is a
> planner policy, not a protocol constraint.

---

## 6. Grounding in the reference maker (`kaleidoswap-maker-rs`)

The taxonomy maps onto maker-rs's existing model with **no wire changes** to the
Boltz-shaped core:

| Taxonomy concept | maker-rs |
|---|---|
| `SwapType` | `submarine` \| `reverse` \| `chain` (`maker-core::SwapType`) — the *shape*, orthogonal to mechanism |
| Leg layer | `Layer` (`BTC_LN`, `BTC_LIQUID`, `BTC_ARKADE`, `LIQUID_LIQUID`; `BTC_L1` dormant) — `#[non_exhaustive]`, so RGB/Spark are additive v2 variants |
| `atomic` claim/refund | `maker-musig` MuSig2 cooperative + tapscript claim/refund leaf |
| `vhtlc` | `maker-layer-arkade` VHTLC lockup (`Action::LockArkade`) |
| `wrapped-hodl` | LN hold invoice primitive (`receive_for_hash`/`claim_for_hash`/`fail_for_hash`), proxy Mode B |
| `coop-send` | the shipped un-gated `BTC↔L-USDT` chain pair |
| Swap lifecycle | `maker-core::SwapState` machine: `Created → AwaitingPayment → PaymentDetected → PaymentConfirmed → Executing → Settled`, with `PendingRateDecision`, `Refunding → Refunded`, `Expired`, `Failed` |
| Preimage reveal | `SwapEvent::HtlcSettled { preimage }` |

**Key relationship:** `SwapType` is the *directional shape* (on-chain→LN,
LN→on-chain, on-chain↔on-chain); **mechanism** is *how each leg locks*. A single
`chain` swap between BTC-L1 and L-USDT can be settled by `boltz-htlc` (atomic,
gated) **or** `coop-send` (trust-minimized, shipped) — same `SwapType`, different
mechanism and tier. The mechanism `id` is therefore additional metadata the pair
card must carry (§7); it is not derivable from `SwapType` alone.

---

## 7. Operator roles (from the wrapped-hold proxy design)

The same primitive serves two operator roles — the taxonomy applies to both:

- **Mode A — counterparty.** Taker swaps against the maker's own inventory across
  two legs linked by one `H`. This is every `submarine`/`reverse`/`chain` swap
  today.
- **Mode B — proxy.** Payer settles in one asset/venue while a *different*
  beneficiary is paid in another (maker as intermediary). `wrapped-hodl`
  generalizes this to any `(inbound venue, outbound venue)` pair. The atomicity
  guarantee is unchanged — one hash, a hash-locked hold per venue.

---

## 8. Taker capability model

The SDK's host declares capabilities; the planner filters mechanisms by them.
Capabilities correspond to what a `WalletPort` implementation can do (see the
SDK design):

| Capability | Meaning |
|---|---|
| `ln:pay` | Pay a BOLT11 invoice |
| `ln:receive` | Issue a BOLT11 invoice the maker can pay |
| `onchain:btc` | Sign/broadcast a BTC-L1 tx and watch an address |
| `onchain:liquid` | Same, Liquid |
| `arkade` | Hold/spend Arkade VTXOs |
| `rgb-ln` | An RGB-LN channel (RLN) — v2 |
| `rgb-l1` | RGB-L1 wallet (rgb-lib) — v2 |
| `spark` | Spark wallet — client-side venue |

The **minimum viable integration is `ln:pay`** — it unlocks `wrapped-hodl` for
every output the operator can deliver. Every other capability progressively
unlocks native, cheaper, or composable mechanisms. This is the packaging story:
capabilities are lazy-loaded modules (physical weight — 13 MB rgb-lib wasm, an
RLN connection), **not** commercial tiers. Monetization tiering lives on the
operator side (maker-in-a-box, hedging), never on client mechanisms.

---

## 9. Versioning & extension

- **`Layer` and `Protocol` are `#[non_exhaustive]`** in maker-core: RGB-L1,
  RGB-LN, and Spark land as new variants without breaking exhaustive matches.
- **Adding a mechanism** = a new `id` in §4's catalog + a new `LegExecutor` in the
  SDK + (if maker-settled) a new `Action` in the maker. No change to `SwapType`,
  the state machine, or the quote/status wire types.
- Operators advertise the set of `(pair, mechanism, atomicity)` they support; an
  SDK on a newer catalog than an operator simply sees fewer mechanisms offered.
  Forward/backward compatible by construction.

---

## 10. What doc 02 must pin down (open items)

Recorded here so the taxonomy stays the stable layer above them:

1. **Pair-card extension** — add `mechanism: MechanismId` and `atomicity: Tier`
   to `PairCard` (today it carries `hash`, `rate`, `limits`, `fees`). Pairs are
   already scoped per `SwapType`; they must additionally be scoped per mechanism
   (a `BTC/L-USDT` chain pair may exist as both `boltz-htlc` and `coop-send`).
2. **Quote request** — `QuoteRequest` gains an optional `minAtomicity` and/or
   explicit `mechanism`; absent → operator picks its best-tier offering.
3. **Multi-hop wire** — how a taker requests/settles a shared-`H` route spanning
   two operators (the two-maker case is implemented server-side; the client-facing
   request/status shape is undefined).
4. **Capability negotiation** — whether the taker sends its capability set at
   quote time so the operator only returns feasible mechanisms, or the SDK filters
   client-side from the full catalogue.
5. **`validUntil` / expiry units** — normative seconds, with a client-side
   sanity bound (a maker returning ms must not yield a 3000-year expiry).
