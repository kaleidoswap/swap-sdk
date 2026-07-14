# Boltz Client Python Bindings

Python bindings for the Boltz Rust library, enabling atomic swaps between Bitcoin, Lightning Network, and Liquid.

## Installation

```bash
pip install kaleidoswap_sdk
```

## Quick Start

> **⚠️ WARNING: All examples are only to be used in REGTEST.**

```python
import kaleidoswap_sdk
import asyncio

async def main():
    # Initialize for regtest (do NOT use this example in production)
    network = kaleidoswap_sdk.Network.REGTEST
    boltz_api = kaleidoswap_sdk.BoltzApiClientV2.default(network)

    # Example: Create a submarine swap (Lightning → Bitcoin)
    key_pair = kaleidoswap_sdk.KeyPair()
    btc_chain = kaleidoswap_sdk.btc_chain_from_network(network)

    invoice = "lightning-invoice-to-pay"

    request = kaleidoswap_sdk.CreateSubmarineRequest(
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

- [`reverse.py`](https://github.com/SatoshiPortal/boltz-rust/blob/trunk/bindings/python/examples/reverse.py) - Lightning to Bitcoin
- [`submarine.py`](https://github.com/SatoshiPortal/boltz-rust/blob/trunk/bindings/python/examples/submarine.py) - Bitcoin to Lightning
- [`chain.py`](https://github.com/SatoshiPortal/boltz-rust/blob/trunk/bindings/python/examples/chain.py) - Bitcoin to Liquid (and vice versa)
