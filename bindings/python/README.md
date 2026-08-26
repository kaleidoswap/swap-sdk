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
    boltz_api = kaleidorg_swap_sdk.BoltzApiClientV2.default(network)

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
production. Without one, `BoltzApiClientV2` behaves exactly as before and
creates unattributed swaps.

```python
import os
import kaleidorg_swap_sdk

client = kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
    "https://maker.signet.kaleidoswap.com/v2",
    os.environ["KALEIDOSWAP_API_KEY"],
    None,  # timeout in seconds
)

client.api_key_environment()  # "test"
client.api_key_id()           # the key id the partner panel shows
```

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
half, and `str(client)` prints the key id and environment only.

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
