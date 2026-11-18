"""Generated async resolver export and satoshi validation, without network I/O."""

import asyncio
import kaleidorg_swap_sdk as sdk


async def run():
    for address, amount in [
        ("alice@example.invalid", 0),
        ("alice@example.invalid", 18446744073709552),
        ("invalid-address", 1),
    ]:
        try:
            await sdk.resolve_lightning_address(address, amount)
        except sdk.Error:
            pass
        else:
            raise AssertionError("invalid resolver input must fail")


asyncio.run(run())
