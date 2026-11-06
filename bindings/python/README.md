# KaleidoSwap SDK Python Bindings

Python bindings for the KaleidoSwap SDK: atomic swaps (Boltz protocol) between Bitcoin, Lightning, and Liquid.

## Installation

```bash
python -m pip install kaleidorg_swap_sdk
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
import kaleidorg_swap_sdk as sdk

network = sdk.Network.SIGNET
kaleido = sdk.KaleidoSwap(sdk.KaleidoSwapOptions(
    network=network,
    store=sdk.FileStore("swaps"),
    signer=sdk.MnemonicSigner(swap_mnemonic, None, network),
))
report = await kaleido.resume()
```

`FileStore` keeps one JSON file per swap, owner-readable only, and hands out
each swap index exactly once; `MemoryStore` keeps nothing past the process. Both
implement `sdk.SwapStore`, and `MnemonicSigner` implements `sdk.SwapSigner`:
subclass either to keep swaps in your own database or derive keys from your
own wallet. Records cross `SwapStore` as JSON strings; persist them verbatim.
They carry the maker's `swap_auth` and the material a refund needs, so treat
the store as secret.

**Resume at startup.** `resume()` claims or refunds every swap the store still
holds and returns a `ResumeReport`: `resumed` (the `Swap`s still in flight),
`settled`, `refunded`, `failed`, and `errors`, the swaps it could not move this
pass, each with an `id`, a `code` and a `message`. Those stay in the store for
the next `resume()`.

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
import kaleidorg_swap_sdk
import asyncio

async def main():
    # Initialize for regtest (do NOT use this example in production)
    network = kaleidorg_swap_sdk.Network.REGTEST
    boltz_api = kaleidorg_swap_sdk.SwapClient.default(network)

    # Example: Create a submarine swap (Lightning → Bitcoin)
    key_pair = kaleidorg_swap_sdk.KeyPair()
    btc_chain = kaleidorg_swap_sdk.btc_chain_from_network(network)

    invoice = "lightning-invoice-to-pay"

    request = kaleidorg_swap_sdk.CreateSubmarineRequest(
        _from=btc_chain,
        to=btc_chain,
        invoice=invoice,
        refund_public_key=key_pair.public(),
    )

    swap = await boltz_api.create_swap(request)
    print(f"Send {swap.expected_amount} sats to {swap.address}")

asyncio.run(main())
```

## Partner attribution from an app (organization keys)

An app attributes its swaps with the organization's public **organization
key**: the 64-hex x-only public key the organization registered in the partner
panel. It is public, so it is safe to ship inside a mobile or desktop app.

```python
import kaleidorg_swap_sdk

client = kaleidorg_swap_sdk.SwapClient.kaleido_org(
    "https://maker.signet.kaleidoswap.com/v2",
    ORG_KEY,  # 64 hex, from the partner panel
    None,  # timeout in seconds
)

client.org_key()  # the key, as 64 lowercase hex
```

The key is a label, never a credential: it buys the caller no rate limit,
discount or access of its own. It goes out in `X-Kaleido-Org-Key` on swap
creation only, to that maker URL only, which must be `https` unless it is a
loopback address. A value that is not a point on the curve is rejected here
rather than reaching the maker as `400 org_key_malformed`. A client holds an
organization key or an API key, never both.

## Partner attribution from a server (organization API keys)

A server can instead use an **organization API key** from the KaleidoSwap partner panel — a
`kld_test_…` key for signet and staging, `kld_live_…` for mainnet and
production. Without one, `SwapClient` behaves exactly as before and
creates unattributed swaps.

```python
import os
import kaleidorg_swap_sdk

client = kaleidorg_swap_sdk.SwapClient.kaleido_maker(
    "https://maker.signet.kaleidoswap.com/v2",
    os.environ["KALEIDOSWAP_API_KEY"],
    None,  # timeout in seconds
)

client.api_key_environment()  # "test"
client.api_key_id()           # the key id the partner panel shows
```

> **Scrub the key in error reporters that capture locals.** It crosses the
> binding as a plain `str`, so it is a function argument on a stack frame for
> the length of the call. The SDK keeps it out of its own errors, logs and
> `repr`, but anything that renders frame locals — `pytest --showlocals`,
> Sentry's `with_locals`, some logging formatters — reads it off the frame
> regardless. Scrub it in your reporter's before-send hook.

The result is an ordinary client — every swap route works the same way — that
sends the key as `Authorization: Bearer …` to that maker URL, and only to that
maker URL. The key answers *which partner organization created this swap?* and
nothing else: it authorizes no claim, no refund, no fund movement and no panel
access. The per-swap `swap_auth` credential the maker returns on create stays
separate and unchanged.

The URL must be `https` unless it is a loopback address, since a bearer
credential over plain HTTP is readable by anything on the path. A value that
cannot be a key is rejected here rather than reaching the maker as a `401` —
which is the same answer a revoked key gets. There is no accessor for the secret
half: `api_key_id()` and `api_key_environment()` are all the client will tell
you, and UniFFI renders no string form of the object at all.

Keep the key in server-side configuration. It is permanent until revoked, so
never ship it inside a mobile or desktop application, where every user holds it.

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
