"""RLN client example.

Build the bindings first (from bindings/):
    make build-debug            # generates kaleidoswap_sdk.py + the native lib

Then run:
    uv run --with pydantic python examples/rln.py

Request/response types are package-bundled pydantic models, converted to/from JSON
across the FFI boundary automatically — you never touch raw JSON.
"""

import asyncio

import kaleidoswap_sdk
from kaleidoswap_sdk import rln_types


async def main() -> None:
    # Point at a running RGB Lightning Node; token is the Biscuit bearer (if set).
    client = kaleidoswap_sdk.RlnClient("http://localhost:3001", None, None)

    # Unlock the node (typed pydantic request in, nothing back).
    await client.unlock(
        rln_types.UnlockRequest(
            password="nodepass",
            bitcoind_rpc_username="user",
            bitcoind_rpc_password="pass",
            bitcoind_rpc_host="localhost",
            bitcoind_rpc_port=18443,
            indexer_url="127.0.0.1:50001",
        )
    )

    # Typed response out: node_info() returns a rln_types.NodeInfoResponse.
    info = await client.node_info()
    print("pubkey:", info.pubkey)

    # RGB: list assets (typed request + response).
    assets = await client.list_assets(
        rln_types.ListAssetsRequest(filter_asset_schemas=[])
    )
    print("assets:", assets)


if __name__ == "__main__":
    asyncio.run(main())
