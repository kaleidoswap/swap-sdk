# KaleidoSwap Operator Protocol — 03: Transport Bindings

**Status:** Draft v0.1 · **Date:** 2026-07-21
**Depends on:** [`01-mechanism-taxonomy`](operator-protocol-01-mechanism-taxonomy.md), [`02-payload-spec`](operator-protocol-02-payload-spec.md)

> The payloads in doc 02 are transport-agnostic. This doc defines the two ways a
> taker's SDK talks to an operator: **HTTP** (the shipped `/v2` surface) and
> **Nostr** (the zero-infrastructure P2P binding). Same request/response bodies;
> different envelope, discovery, and identity model.
>
> The Nostr binding follows Electrum's submarine-swap provider model
> (`electrum/submarine_swaps.py`: NIP-38-style announcements + PoW anti-spam +
> encrypted DMs), adapted for KaleidoSwap's multi-mechanism payloads and hardened
> (NIP-44, not the deprecated NIP-04).

---

## 1. The `SwapTransport` seam

Both bindings implement one SDK interface, so the planner/executor are
transport-blind:

```ts
interface SwapTransport {
  pairs(swapType): Promise<PairCatalogue>        // doc 02 §3
  quote(req: QuoteRequest): Promise<QuoteResponse>
  create(req: CreateRequest): Promise<CreateResponse>
  cooperative(id, req): Promise<CooperativeSignatureResponse>
  status(id): Promise<SwapStatus>
  subscribe(id, cb): Unsubscribe                 // push updates
}
interface Operator { id: OperatorId; transport: SwapTransport }
```

`HttpTransport` and `NostrTransport` are the two implementations. An operator's
`id` is its **Nostr pubkey** (canonical) with an optional HTTPS base URL — so the
same operator is reachable over either binding, and identity is the key, not the
URL. This mirrors Electrum's `SwapServerTransport` abstraction with `Http` and
`Nostr` implementations.

---

## 2. HTTP binding

Direct mapping of doc 02 onto the reference maker's `/v2` routes:

| SDK call | HTTP |
|---|---|
| `pairs(type)` | `GET /v2/swap/{type}/pairs` |
| `quote` | `POST /v2/quote` |
| `create` (submarine/reverse/chain) | `POST /v2/swap/{submarine,reverse,chain}` |
| `cooperative` | `POST /v2/swap/{id}/{claim,refund}` (per type) |
| `status` | `GET /v2/swap/{id}` |
| `subscribe` | `WS /v2/ws` (fan-out) |

- **Identity:** the base URL's TLS cert + the `serverPublicKey` returned on
  create. The SDK MUST verify `swapTree` against the amounts/keys before funding
  regardless of transport (the operator is never trusted for atomicity).
- **TLS required.** No secrets in URLs or query strings (SDK security rule).
- Best for: hosted operators with infrastructure (the default KaleidoSwap maker),
  server-to-server, CI/headless.

---

## 3. Nostr binding — the zero-infrastructure P2P story

A market maker needs **only a Nostr keypair** to be discoverable and reachable —
no domain, no TLS cert, no inbound port, NAT/home-node friendly. This is what
makes "P2P liquidity" a demo rather than a slide.

### 3.1 Operator announcements (discovery)

Operators publish a **replaceable** offer event, refreshed on an interval:

```jsonc
{
  "kind": 31555,                     // [PROPOSED] parameterized-replaceable (30000–39999)
  "pubkey": "<operator pubkey>",
  "tags": [
    ["d", "kswap-operator-v1"],      // filterable discriminator (our own, not Electrum's)
    ["r", "net:mainnet"],            // network scoping (mainnet | signet | regtest)
    ["expiration", "1737483600"]     // NIP-40; announcement TTL
  ],
  "content": "{ ...OfferContent... }" // JSON, see below
}
```

`OfferContent` (superset of Electrum's — we advertise the mechanism matrix, doc
02 §3):

```jsonc
{
  "pairs": [
    { "from": "BTC@LN", "to": "L-USDT", "mechanism": "wrapped-hodl",
      "atomicity": "atomic", "min": 10000, "max": 25000000,
      "feePpm": 5000, "minerFees": 180 },
    { "from": "BTC@LN", "to": "BTC@ARK", "mechanism": "vhtlc",
      "atomicity": "atomic", "min": 5000, "max": 10000000, "feePpm": 3000 }
  ],
  "relays": ["wss://relay.kaleidoswap.com", "wss://…"],  // where to reach this operator
  "powNonce": "…",                   // anti-spam, §3.4
  "minVersion": "0.1",               // protocol version floor
  "capabilities": ["ln:pay","onchain:liquid","arkade"]   // what this operator can settle
}
```

> The full `PairCard` (with `hash` rate-lock) is fetched via a `pairs` request
> (§3.3) before quoting — the announcement carries an *indicative* rate/limit
> summary for discovery and ranking, not the binding rate card. This keeps
> announcements small and rate cards fresh.

### 3.2 Discovery & selection (client)

Subscribe with a filter on the discriminator + network:

```jsonc
{ "kinds": [31555], "#d": ["kswap-operator-v1"], "#r": ["net:mainnet"],
  "since": <now - 3600> }
```

Then, per Electrum's approach: keep the freshest event per operator pubkey, drop
announcements failing the **PoW threshold** (§3.4), and rank the survivors. The
SDK ships **KaleidoSwap's operator pubkey pinned** as the default; discovered
third-party operators are used only when the host opts in (`trustDiscovered:
true`) and are always subject to per-operator policy caps (the wallet-engine
policy engine).

### 3.3 Request / response (protocol messages)

Interactive calls (`pairs`, `quote`, `create`, `cooperative`, `status`,
`rate-decision`) ride over **NIP-44-encrypted, ephemeral** events so relays never
persist swap traffic:

```jsonc
{
  "kind": 25555,                     // [PROPOSED] ephemeral (20000–29999): not stored by relays
  "pubkey": "<taker ephemeral pubkey>",
  "tags": [["p", "<operator pubkey>"]],
  "content": "<NIP-44 encrypt({ method, id?, body }) >"
}
```

- **Plaintext (encrypted) body:** `{ "method": "quote" | "create" | "pairs" |
  "cooperative" | "status" | "rate-decision", "body": <doc-02 payload> }`.
- **Correlation:** the operator's reply is an ephemeral event tagged
  `["e", <request event id>]` (Electrum's `reply_to`), encrypted to the taker's
  ephemeral pubkey. The SDK matches replies to pending requests by that id.
- **Per-swap ephemeral identity:** the taker SHOULD use a fresh key per swap
  (unlinkability); it is *not* a wallet key.

### 3.4 Anti-spam: proof-of-work

Adopt Electrum's scheme wholesale: the operator computes a PoW `nonce` over its
pubkey; the client verifies `powBits(pubkey, nonce) ≥ SWAPSERVER_POW_TARGET` and
rejects announcements below the configured target. Cheap, proven, no registry or
gatekeeper. PoW is a **sybil dampener for discovery**, not authentication —
authentication is the pubkey + the trustless `swapTree` verification.

### 3.5 Status streaming

`subscribe(id, cb)` = the taker stays subscribed to reply events tagged with the
swap id; the operator pushes each `SwapStatus` transition (doc 02 §7) as a new
encrypted ephemeral event. This replaces the HTTP `WS /v2/ws` fan-out; no polling
needed while the SDK is online. `status(id)` (a `method: "status"` request) is the
resume-after-offline fallback.

---

## 4. Binding parity & differences

| Concern | HTTP | Nostr |
|---|---|---|
| Identity | TLS cert + `serverPublicKey` | operator pubkey (canonical) |
| Discovery | out-of-band (known URL) | announcement events + PoW |
| Infra to be an operator | domain, TLS, inbound port | a keypair + a relay to publish to |
| Push updates | `WS /v2/ws` | subscription to reply events |
| Privacy | TLS to a known host | encrypted, ephemeral (unstored); fresh key per swap |
| Errors (doc 02 §10) | HTTP status + string | `{ error: { code, message } }` in the encrypted reply |

**Payloads are identical.** A maker implementing plain Boltz `/v2` over HTTP is
upgradeable to the Nostr binding by adding an announcer + a DM listener that feeds
the *same* handlers — no rewrite. (In `kaleidoswap-maker-rs` this is two small
modules alongside `maker-api`; the swap engine is untouched.)

---

## 5. Security

- **Trustless atomicity is transport-independent:** the SDK verifies `swapTree`
  (claim/refund leaves, amounts, timeouts) before funding on *either* binding. A
  malicious operator (HTTP or Nostr) can grief (quote-and-vanish) but cannot steal
  — the refund leaf backs the `atomic` tier.
- **No key material in events** beyond the NIP-44-encrypted body; `preimage` is
  never sent (revealed only on-chain/in-HTLC at claim, doc 02 §1).
- **Relay trust is minimal:** relays see only ephemeral, encrypted blobs and the
  `p`-tag routing metadata; they cannot read or persist swap contents. Operators
  list multiple relays for fault tolerance (Electrum-style relay-list persistence).
- **Replay/expiry:** announcements carry NIP-40 `expiration`; quotes carry
  `validUntil` (doc 02 §4) with a client-side sanity bound; ephemeral request
  events are single-use (correlated by id).
- **`[MV3]`** In the browser-extension host, NIP-44 crypto and relay I/O MUST run
  off the JS critical path (worker / async), per the rate NWC lesson where
  synchronous Nostr crypto froze the UI thread.

---

## 6. KaleidoSwap infrastructure hooks

- **Default relay:** `wss://relay.kaleidoswap.com` (operated) — day-one publish +
  subscribe target; the relay position has standalone network value.
- **Human-readable operator identity:** NIP-05 via `kaleidoswap.me`
  (`maker@kaleidoswap.com` → operator pubkey) for display and pinning.
- **Existing crypto:** the extension already has Nostr identity (nsec-rooted
  wallets, NIP-07 signer) and NWC plumbing — the `NostrTransport` reuses it.

---

## 7. Sequencing (from the SDK plan)

- **Phase 1:** `HttpTransport` only, single pinned KaleidoSwap operator. The
  `SwapTransport` seam exists from the start so the Nostr binding is additive.
- **Phase 2a:** publish docs 01–02 + this HTTP binding; certify makers against
  `/v2`.
- **Phase 2b:** `NostrTransport` in the SDK + announcer/DM-listener modules in
  `kaleidoswap-maker-rs`; demo a second operator discovered over Nostr → "add an
  MM" becomes real.

> Kind numbers `31555` / `25555` are **proposals** — pick final values and
> reserve them in a short NIP-style note before Phase 2b so third-party makers and
> Electrum-adjacent tooling can interoperate.
