# KaleidoSwap SDK Python Bindings

Python bindings for the KaleidoSwap SDK: atomic swaps (Boltz protocol) between Bitcoin, Lightning, and Liquid.

## Installation

```bash
python -m pip install kaleidorg_swap_sdk
```

Python 3.10+ is supported. Prebuilt wheels cover Linux x86_64/aarch64, macOS
x86_64/arm64, and Windows x86_64; other platforms build the published source
distribution and need a Rust 1.88+ toolchain.

## Quick Start

> **⚠️ WARNING: All examples are only to be used in REGTEST.**

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

## Partner attribution (organization API keys)

A partner organization can have the swaps it originates attributed to it. That
needs an **organization API key** from the KaleidoSwap partner panel — a
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

- **Submarine swaps** - Lightning → On-chain Bitcoin/Liquid
- **Reverse swaps** - On-chain Bitcoin/Liquid → Lightning
- **Chain swaps** - Bitcoin ↔ Liquid atomic swaps

## Examples

Complete working examples are available in the `examples/` directory:

- [`reverse.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/reverse.py) - Lightning to Bitcoin
- [`submarine.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/submarine.py) - Bitcoin to Lightning
- [`chain.py`](https://github.com/kaleidoswap/swap-sdk/blob/trunk/bindings/python/examples/chain.py) - Bitcoin to Liquid (and vice versa)

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

[The adapter example](examples/rgb_spend.py) demonstrates both spend directions.
It requires an application-provided RGB wallet adapter; live rgb-lib validation
is the next integration phase.
