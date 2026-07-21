import asyncio

from common import *


async def swap(from_chain: kaleidoswap_sdk.Chain, refund: bool):
    ws_client = boltz_api.ws()

    key_pair = kaleidoswap_sdk.KeyPair()

    invoice = await new_invoice(50000)

    request = kaleidoswap_sdk.CreateSubmarineRequest(
        _from=from_chain,
        to=btc_chain,
        invoice=invoice,
        refund_public_key=key_pair.public(),
    )

    swap_response = await boltz_api.create_swap(request)
    swap_script = kaleidoswap_sdk.SwapScript.from_submarine(
        from_chain, swap_response, key_pair.public()
    )
    swap_id = swap_response.id

    asyncio.create_task(ws_client.run_ws_loop())

    # Monitor the swap status via WebSocket
    updates = ws_client.updates()
    await ws_client.subscribe_swap(swap_id)

    # Wait for initial status
    await next_status(updates, "invoice.set")

    # Send the expected amount to the lockup address
    txid = await sendtoaddress(
        from_chain,
        swap_response.address,
        swap_response.expected_amount / 2 if refund else swap_response.expected_amount,
    )

    if refund:
        await next_status(updates, "transaction.lockupFailed")
        await delay()

        refund_address = await getnewaddress(from_chain)

        tx = await swap_script.construct_refund(
            kaleidoswap_sdk.SwapTransactionParams(
                output_address=refund_address,
                fee=kaleidoswap_sdk.Fee.ABSOLUTE(200),
                swap_id=swap_id,
                keys=key_pair,
                chain_client=chain_client,
                boltz_api=boltz_api,
            )
        )

        txid = await chain_client.broadcast_tx(tx)
        assert txid is not None
    else:
        # Wait for transaction to appear in mempool
        await next_status(updates, "transaction.mempool")
        await mine_block()

        await next_status(updates, "transaction.claim.pending")
        await swap_script.submarine_cooperative_claim(
            swap_id, key_pair, invoice, boltz_api
        )

        await next_status(updates, "transaction.claimed")
        await delay()


async def main():
    await swap(btc_chain, False)
    await swap(lbtc_chain, False)
    await swap(btc_chain, True)
    await swap(lbtc_chain, True)


if __name__ == "__main__":
    asyncio.run(main())
