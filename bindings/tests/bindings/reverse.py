import kaleidoswap_sdk
import asyncio
from common import *


async def swap(to_chain: kaleidoswap_sdk.Chain):
    boltz_api = kaleidoswap_sdk.BoltzApiClientV2.default(network)
    # Initialize WebSocket client
    ws_client = boltz_api.ws()

    # Generate a new key pair for the swap
    key_pair = kaleidoswap_sdk.KeyPair()

    # Get the amount to swap from user
    amount = 1000000

    preimage = kaleidoswap_sdk.Preimage()

    # Create a reverse swap request
    request = kaleidoswap_sdk.CreateReverseRequest(
        invoice_amount=amount,
        _from=btc_chain,
        to=to_chain,
        preimage_hash=preimage.sha256(),
        claim_public_key=key_pair.public(),
    )

    response = await boltz_api.create_reverse_swap(request)
    swap_id = response.id

    asyncio.create_task(ws_client.run_ws_loop())

    # Monitor the swap status via WebSocket
    await ws_client.subscribe_swap(swap_id)
    updates = ws_client.updates()

    # Wait for initial status
    await next_status(updates, "swap.created")

    # Pay the invoice
    asyncio.create_task(pay_invoice(response.invoice))

    # Wait for transaction to appear in mempool
    await next_status(updates, "transaction.mempool")

    await delay()

    swap_script = kaleidoswap_sdk.SwapScript.from_reverse(
        chain=to_chain, reverse_response=response, our_pubkey=key_pair.public()
    )

    claim_address = await getnewaddress(to_chain)
    tx = await swap_script.construct_claim(
        preimage,
        kaleidoswap_sdk.SwapTransactionParams(
            swap_id=swap_id,
            keys=key_pair,
            fee=kaleidoswap_sdk.Fee.ABSOLUTE(200),
            output_address=claim_address,
            chain_client=chain_client,
            kaleidoswap_sdk=boltz_api,
        ),
    )

    await chain_client.broadcast_tx(tx)

    await next_status(updates, "invoice.settled")


async def main():
    await swap(btc_chain)
    await swap(lbtc_chain)


if __name__ == "__main__":
    asyncio.run(main())
