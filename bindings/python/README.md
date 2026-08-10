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

## Swap Types

- **Submarine swaps** - Lightning → On-chain Bitcoin/Liquid
- **Reverse swaps** - On-chain Bitcoin/Liquid → Lightning
- **Chain swaps** - Bitcoin ↔ Liquid atomic swaps

## Examples

Complete working examples are available in the `examples/` directory:

- [`reverse.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/reverse.py) - Lightning to Bitcoin
- [`submarine.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/submarine.py) - Bitcoin to Lightning
- [`chain.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/chain.py) - Bitcoin to Liquid (and vice versa)
