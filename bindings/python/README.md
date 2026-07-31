# KaleidoSwap SDK Python Bindings

Python bindings for the KaleidoSwap SDK: atomic swaps (Boltz protocol) between Bitcoin, Lightning, and Liquid.

## Installation

Public PyPI's normalized `kaleidoswap-sdk` project belongs to an older release
line, so an unqualified `pip install kaleidoswap_sdk==0.1.0` does **not** install
this repository's v0.1.0 package.

Install a platform wheel attached to the GitHub `v0.1.0` release:

```bash
python -m pip install ./kaleidoswap_sdk-0.1.0-<platform>.whl
```

If the optional TestPyPI release is enabled, download this package from
TestPyPI without dependency resolution, then install that exact file so runtime
dependencies resolve from the normal index:

```bash
python -m pip download --no-deps \
  --index-url https://test.pypi.org/simple/ \
  kaleidoswap_sdk==0.1.0
python -m pip install ./kaleidoswap_sdk-0.1.0-<platform>.whl
```

Python 3.10+ is supported on Linux x86_64/aarch64, macOS x86_64/arm64, and
Windows x86_64.

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

- [`reverse.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/reverse.py) - Lightning to Bitcoin
- [`submarine.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/submarine.py) - Bitcoin to Lightning
- [`chain.py`](https://github.com/kaleidoswap/kaleidoswap-sdk/blob/trunk/bindings/python/examples/chain.py) - Bitcoin to Liquid (and vice versa)
