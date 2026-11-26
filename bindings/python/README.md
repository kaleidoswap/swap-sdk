# KaleidoSwap SDK Python Bindings

Python bindings for the KaleidoSwap SDK: atomic swaps (Boltz protocol) between Bitcoin, Lightning, and Liquid.

## Installation

```bash
python -m pip install kaleidoswap
```

Python 3.10+ is supported. Prebuilt wheels cover Linux x86_64/aarch64, macOS
x86_64/arm64, and Windows x86_64; other platforms build the published source
distribution and need a Rust 1.88+ toolchain.

## High-level API: `KaleidoSwap`

For a swap rather than a protocol. `quote` prices a route, `start` creates the
swap and returns what to pay or fund, and `watch` follows the swap to the end,
claiming or refunding on its own; `resume` finishes or refunds whatever a
previous run left in the store. The store and the signer are yours: the SDK
holds no key material and never funds anything on your behalf.

```python
import kaleidoswap as sdk

network = sdk.Network.SIGNET
# One FileStore and one KaleidoSwap per store directory per process.
kaleido = sdk.KaleidoSwap(sdk.KaleidoSwapOptions(
    network=network,
    store=sdk.FileStore(store_dir),
    signer=sdk.MnemonicSigner(swap_mnemonic, None, network),
))
report = await kaleido.resume()
```

`FileStore` keeps one JSON file per swap, owner-readable only. Give it a
directory under your application's data directory, outside any source tree and
out of version control. `MemoryStore` keeps nothing past the process: use it in
tests only, never for a swap you fund. Both implement `sdk.SwapStore`, and
`MnemonicSigner` implements `sdk.SwapSigner`: subclass either to keep swaps in
your own database or sign with your own wallet. A `SwapSigner` is asked for
public keys, preimages and signatures (`public_key`, `derive_preimage`,
`sign_schnorr`, `musig_nonce`, `musig_partial_sign`, hex in and out), never for
a secret key, so the keys can stay in a hardware wallet or a remote signer; it
keeps each MuSig2 secret nonce itself and signs with it once. Records cross
`SwapStore` as JSON strings; persist them verbatim. They carry the maker's
`swap_auth` and the material a refund needs, so treat the store as secret and
do not log records or send them to telemetry.

**Store rules.**

- A store belongs to exactly one swap mnemonic.
- Use one `FileStore` and one `KaleidoSwap` per store directory per process.
- Never reset, copy or restore a store from an older backup while its swap
  mnemonic is in use.
- If a store is lost, start a new swap mnemonic.
- On a phone, keep the store directory out of device and cloud backups, so
  that a restore never brings back an older copy of it.

A `SwapStore` subclass follows the same rules, and:

- `put` does not return until the record is durably stored.
- `next_index` persists the advanced counter before it returns, is atomic
  across callers, and never goes backwards.

**Resume at startup.** `resume()` moves every swap the store still holds one
step (claiming what is claimable, refunding what is refundable) and returns a
`ResumeReport`: `resumed` (the `Swap`s still in flight), `settled`, `refunded`,
`failed`, and `errors`, the swaps it could not move this pass, each with an
`id`, a `code` and a `message`. Those stay in the store for the next
`resume()`. Follow each swap in `resumed` to the end with `watch()` or
`settled()`; `resume()` does not wait for funding or confirmations.

### Quote

Both sides are yours to name, each an `sdk.Asset`: what it is and where it lives.

| `sdk.Asset`                  | On the wire       |
| ---------------------------- | ----------------- |
| `BTC_ONCHAIN()`              | `bitcoin:BTC`     |
| `BTC_LIGHTNING()`            | `lightning:BTC`   |
| `BTC_LIQUID()`               | `liquid:BTC`      |
| `USDT_LIQUID()`              | `liquid:USDT`     |
| `BTC_ARKADE()`               | `arkade:BTC`      |
| `BTC_SPARK()`                | `spark:BTC`       |
| `RGB_ONCHAIN(contract_id)`   | `bitcoin:rgb:…`   |
| `RGB_LIGHTNING(contract_id)` | `lightning:rgb:…` |
| `SPARK_TOKEN(token_id)`      | `spark:btkn…`     |

`contract_id` is the full RGB contract id, `rgb:` prefix included.

The protocol follows from the two sides and is reported on the quote, never
inferred from an address. The amount says what it fixes, and each route is
priced by one kind:

| Route                                                 | Protocol          | `sdk.Amount`                                               |
| ----------------------------------------------------- | ----------------- | ---------------------------------------------------------- |
| `BTC_LIGHTNING()` → `BTC_ONCHAIN()` or `BTC_LIQUID()` | reverse (Boltz)   | `PAY(sats)` (what you pay) or `RECEIVE(sats)` (what lands) |
| `BTC_ONCHAIN()` or `BTC_LIQUID()` → `BTC_LIGHTNING()` | submarine (Boltz) | `INVOICE(bolt11)`: the invoice the maker pays              |

The wrong kind for a route raises `sdk.Error.Swap` (`INVALID_AMOUNT` or
`INVALID_DESTINATION`) before anything is sent.

```python
answer = await kaleido.quote(sdk.QuoteRequest(
    pay=sdk.Asset.BTC_LIGHTNING(),
    receive=sdk.Asset.BTC_LIQUID(),
    amount=sdk.Amount.PAY(50_000),
))
if answer.refusal:
    raise SystemExit(f"{answer.refusal.reason}: {answer.refusal.message}")
quote = answer.quote
print(quote.protocol, quote.from_amount, quote.to_amount, quote.fees.maker, quote.fees.miner)
```

`quote()` returns a `QuoteAnswer` with exactly one of `quote` and `refusal`
set. A refusal is a value, not an error: the maker will not trade this pair or
this amount. `reason` is a `sdk.QuoteRefusalReason` (`PAIR_UNAVAILABLE`,
`AMOUNT_BELOW_MIN`, `AMOUNT_ABOVE_MAX`, `NO_LIQUIDITY`, `MAKER_PAUSED`),
`limits` carries the bounds when the amount was the problem, and `message`
says why. Routes this layer does not create swaps for yet (chain swaps,
L-USDT, RGB, Arkade, Spark) are a `PAIR_UNAVAILABLE` refusal whose message
names where they live.

### Start, act, watch

```python
class PrintPhases(sdk.PhaseListener):
    def on_phase(self, phase):
        print(phase.name.lower())

swap = await kaleido.start(
    quote, sdk.StartOptions(destination=liquid_address, refund_address=None)
)
action = swap.action()
print(f"pay {action.invoice}")  # a swap from Lightning
# print(f"send {action.funding_amount} sat to {action.funding_address}")  # a swap from chain

try:
    record = await swap.watch(PrintPhases())
except sdk.Error.Swap as error:
    print(f"swap {swap.id()} ended without settling: {error.code} {error.message}")
```

- `start(quote, StartOptions(destination, refund_address))` creates the swap
  and persists it before returning. `destination` is the address the received
  asset lands on, checked against the asset and the network first; leave it
  `None` for a swap priced by an invoice, which is its own destination.
  `refund_address` is required when you pay from chain and ignored when you pay
  over Lightning.
- `swap.action()` is what you do: pay `invoice` for a swap from Lightning, or
  send exactly `funding_amount` sats to `funding_address` for a swap from
  chain. That is the only step that is yours; the claim and the refund are the
  SDK's.
- `swap.watch(listener)` follows the swap to the end, claiming or refunding as
  the maker and the chain move, calls `listener.on_phase` on every change, and
  returns the `SwapRecord` on `SETTLED`. On `REFUNDED` or `FAILED` it raises
  `sdk.Error.Swap`. `swap.settled()` is the same without a listener. A swap
  moves only while something watches it (or `resume()` runs), so keep watching
  until it ends, and resume at the next start if the process stops first.
- `swap.phase()`, `swap.record()` and `kaleido.swap(id)` read state without
  waiting; `swap.tick()` makes one observation and acts on it.

### Phases

Every protocol moves through one `sdk.SwapPhase` machine. The protocol's own
status stays readable as `swap.record().raw_status`.

```
CREATED ─▶ AWAITING_FUNDING ─▶ FUNDED ─▶ LOCKED ─▶ CLAIMING ─▶ SETTLED
                                           │
                                           ├─▶ REFUNDABLE ─▶ REFUNDING ─▶ REFUNDED
                                           └─▶ FAILED
```

`SETTLED`, `REFUNDED` and `FAILED` are terminal. A swap that becomes
`REFUNDABLE` is refunded by the SDK on its own, cooperatively or after the
timeout; `swap.refund()` is there for a caller who wants to decide.

Runnable versions are in [`examples/simple/`](#examples).

## Quick Start

> **⚠️ WARNING: The engine-level examples are only to be used in REGTEST.**

```python
import kaleidoswap
import asyncio

async def main():
    # Initialize for regtest (do NOT use this example in production)
    network = kaleidoswap.Network.REGTEST
    boltz_api = kaleidoswap.SwapClient.default(network)

    # Example: Create a submarine swap (Lightning → Bitcoin)
    key_pair = kaleidoswap.KeyPair()
    btc_chain = kaleidoswap.btc_chain_from_network(network)

    invoice = "lightning-invoice-to-pay"

    request = kaleidoswap.CreateSubmarineRequest(
        _from=btc_chain,
        to=btc_chain,
        invoice=invoice,
        refund_public_key=key_pair.public(),
    )

    swap = await boltz_api.create_swap(request)
    print(f"Send {swap.expected_amount} sats to {swap.address}")

asyncio.run(main())
```

## Partner attribution and the partner fee

A partner's key from the KaleidoSwap partner panel — `kld_test_…` for signet and
staging, `kld_live_…` for mainnet and production — attributes the swaps a
client creates to the partner and carries the partner's fee, set on the key in
the panel. Without one, `SwapClient` behaves exactly as before: public price,
unattributed swaps.

The key is a **public partner identifier**: ship it in the app or the service as
a build constant. It never gives a lower price, higher limits or a rate-limit
budget of its own, so anyone who copies it gains nothing.

```python
import kaleidoswap

# Public: safe to commit and to ship.
KALEIDOSWAP_PARTNER_KEY = "kld_live_…"

client = kaleidoswap.SwapClient.kaleido_maker(
    "https://maker.kaleidoswap.com/v2",
    KALEIDOSWAP_PARTNER_KEY,
    None,  # timeout in seconds
)

client.api_key_environment()  # "live"
client.api_key_id()           # the key id the partner panel shows
```

The result is an ordinary client — every swap route works the same way — that
sends the key as `Authorization: Bearer …` on every request to that maker URL,
and only to that maker URL. That includes the pair cards: a keyed card's
`fees.percentage` already includes the partner fee (`fees.partner_percentage`
is that fee) and its `pair_hash` covers it, so compute amounts from
`percentage` as before and create with the keyed card's hash. The high-level
`KaleidoSwap` takes the same key as `KaleidoSwapOptions(api_key=…)`, and its
quotes show the partner fee as `fees.partner`.

The partner fee comes only from the key: there is no per-request fee option.

**What the maker answered.** A key the maker does not recognise — unknown,
revoked, or for the other environment — is not an error: the maker answers with
the public price and creates the swap unattributed, so an app with a rotated
key keeps swapping. `client.last_attribution()` and
`KaleidoSwap.last_attribution()` return what the maker answered in
`X-Kaleido-Attribution`: `"api_key"` when it priced or stored the request for
the key's partner, `"none"` when it did not recognise the key.

The key authorizes nothing: no claim, no refund, no fund movement and no panel
access. The per-swap `swap_auth` credential the maker returns on create stays
separate and unchanged. The URL must be `https` unless it is a loopback
address, since over plain HTTP anything on the path could strip or replace the
key. A value that cannot be a key is rejected here.

## Swap Types

- **Submarine swaps** - On-chain Bitcoin/Liquid → Lightning
- **Reverse swaps** - Lightning → On-chain Bitcoin/Liquid
- **Chain swaps** - Bitcoin ↔ Liquid atomic swaps

## Examples

Complete working examples are in the `examples/` directory, in two folders.

**`examples/simple/`** uses the high-level `KaleidoSwap` surface: you name what
you pay and what you receive, and the SDK prices the route, checks the maker's
response, persists the swap, and claims or refunds on its own. They run against
the signet maker. Start here.

- [`simple/submarine.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/simple/submarine.py) - on-chain Bitcoin to a Lightning invoice
- [`simple/reverse.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/simple/reverse.py) - a Lightning payment to L-BTC on Liquid

**`examples/advanced/`** drives the swap engine step by step: create requests,
scripts, claim and refund transactions, the status WebSocket. Use it when you
own those steps yourself, or for a route the high-level surface does not serve
yet. These run against regtest.

- [`advanced/submarine.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/advanced/submarine.py) - Bitcoin to Lightning
- [`advanced/reverse.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/advanced/reverse.py) - Lightning to Bitcoin
- [`advanced/chain.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/advanced/chain.py) - Bitcoin to Liquid (and vice versa)
- [`advanced/corridor_quote.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/advanced/corridor_quote.py) - the Arkade Intents corridor
- [`advanced/rgb_spend.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/advanced/rgb_spend.py) - claim and refund helpers for an rgb-lib wallet

## USDT-RGB swaps

Set `rgb_contract_id` on `CreateSubmarineRequest` or `CreateReverseRequest` and
select `Currency.USDT_RGB` for the on-chain side. The pin is required before the
POST and checked against the returned contract. It stays local; the maker wire
request is unchanged. RGB chain swaps are unsupported. Other currencies retain
the existing create defaults.

The caller's rgb-lib wallet funds submarine locks and validates reverse lock
consignments. The SDK validates the HTLC, builds the claim/refund PSBT and signs
its script path. RGB proof validation and wallet state remain in rgb-lib.

Lock consignments move over the maker API, not an RGB proxy. A submarine taker
locks with no transport endpoint and posts the consignment with
`client.post_submarine_rgb_lock(swap_id, RgbLockConsignment(consignment=…,
txid=…, vout=…), swap_auth)`. A reverse taker reads the maker's with
`client.get_reverse_rgb_lock(swap_id, swap_auth)` and accepts it with rgb-lib's
`accept_htlc_consignment`.

`SwapScript.prepare_rgb_claim` / `prepare_rgb_refund` take `RgbPsbtParams`:
`output_address`, `funding`, `max_fee` (sats), `swap_id`, `chain_client`,
`boltz_api`, and optional `lockup_tx`. Parse a lock transaction with
`BtcLikeTransaction.from_hex_bitcoin(hex)`. Use the original lock transaction
for refunds. The payout address must come from an RGB wallet witness receive.

- `RgbSpendFunding.HTLC_VALUE(fee_rate_sat_vb=rate)` uses the HTLC's sats.
  Reverse locks are sized for the advertised `rgb.claim_fee_rate`.
- `RgbSpendFunding.CALLER_INPUTS()` requires BTC funding. Add wallet inputs/change
  without coloring or signing, then retain `spend.fund(funded_psbt)`.
  Its `template()` returns the frozen PSBT and current HTLC input index.

Color that PSBT with rgb-lib's `psbt_op_prepare_with_expiry`, assigning the full
RGB amount to the template's payment output. Return the actual RGB allocations
in `ColoredRgbPsbt` to `finalize_claim(colored, keys, preimage)` or
`finalize_refund(colored, keys)`. `FinalizedRgbSpend` contains `psbt`,
`swap_input_index` and optional `transaction`. Sign/finalize remaining wallet
inputs while preserving the SDK's HTLC witness. `Error.RgbFeeInputRequired`
means the HTLC cannot pay the fee while keeping the minimum payout; prepare with
caller inputs instead.

Keep the rgb-lib operation ID and complete `psbt_op_mark_broadcast`, broadcast,
`psbt_op_apply` and `psbt_op_provide_receive_consignment`. Enforce reverse lock
confirmations using the chain and rgb-lib: there is no reverse
`transaction.confirmed` event. Refunds wait for the timeout; claims must confirm
before it. Asset amounts use contract units; `htlc_sat` and fees use sats.


Submarine BTC collateral is capped at 1000 sats by default, independently of
`maxFee`/`max_fee`. The maker receives all of it on a successful claim. Choose
any larger cap locally before reading the response: set `rgb_max_htlc_sat` on `CreateSubmarineRequest`.
The cap is never sent to the maker. RGB spend preparation requires the actual
colored lock transaction; address discovery can select unrelated BTC outputs.
Before paying a reverse invoice, compare its `claimFeeRate` with your current
fee estimates and remaining timeout; reject an inadequate quote. If fees rise,
use a higher spend rate and wallet BTC inputs when needed. A maker fee quote
does not guarantee confirmation before the timeout. Contract-id chunk dashes
are cosmetic; pin a valid id from your trusted wallet or asset registry.

[The adapter example](examples/advanced/rgb_spend.py) demonstrates both spend directions.
It requires an application-provided RGB wallet adapter. Actual RGB wallet validation
is recorded in the [native regtest example](../../examples/rgb-regtest/README.md).
