import asyncio
import kaleidoswap_sdk
from common import *


async def swap(
    from_chain: kaleidoswap_sdk.Chain,
    to_chain: kaleidoswap_sdk.Chain,
    refund: bool = False,
):
    claim_address = await getnewaddress(to_chain)

    ws_client = boltz_api.ws()
    claim_keys = kaleidoswap_sdk.KeyPair()
    refund_keys = kaleidoswap_sdk.KeyPair()
    amount = 1000000
    preimage = kaleidoswap_sdk.Preimage()
    request = kaleidoswap_sdk.CreateChainRequest(
        _from=from_chain,
        to=to_chain,
        preimage_hash=preimage.sha256(),
        claim_public_key=claim_keys.public(),
        refund_public_key=refund_keys.public(),
        user_lock_amount=amount,
    )

    response = await boltz_api.create_chain_swap(request)
    swap_id = response.id

    asyncio.create_task(ws_client.run_ws_loop())

    lockup_script = kaleidoswap_sdk.SwapScript.from_chain(
        chain=from_chain,
        side=kaleidoswap_sdk.Side.LOCKUP,
        chain_swap_details=response.lockup_details,
        our_pubkey=refund_keys.public(),
    )

    claim_script = kaleidoswap_sdk.SwapScript.from_chain(
        chain=to_chain,
        side=kaleidoswap_sdk.Side.CLAIM,
        chain_swap_details=response.claim_details,
        our_pubkey=claim_keys.public(),
    )

    # Monitor the swap status via WebSocket
    await ws_client.subscribe_swap(swap_id)
    updates = ws_client.updates()

    await next_status(updates, "swap.created")

    # Send amount based on whether we're testing refund or not
    send_amount = amount // 2 if refund else amount
    await sendtoaddress(from_chain, response.lockup_details.lockup_address, send_amount)

    if refund:
        await next_status(updates, "transaction.lockupFailed")

        await delay()

        refund_address = await getnewaddress(from_chain)

        refund_params = kaleidoswap_sdk.SwapTransactionParams(
            output_address=refund_address,
            fee=kaleidoswap_sdk.Fee.ABSOLUTE(200),
            swap_id=swap_id,
            keys=refund_keys,
            chain_client=chain_client,
            boltz_api=boltz_api,
        )

        refund_tx = await lockup_script.construct_refund(refund_params)
        txid = await chain_client.broadcast_tx(refund_tx)
        print(f"Refund Transaction ID: {txid}")
        assert txid is not None
    else:
        await next_status(updates, "transaction.mempool")
        await mine_block()

        await next_status(updates, "transaction.confirmed")

        await next_status(updates, "transaction.server.mempool")
        await mine_block()

        await next_status(updates, "transaction.server.confirmed")
        await delay()

        claim_params = kaleidoswap_sdk.SwapTransactionParams(
            output_address=claim_address,
            fee=kaleidoswap_sdk.Fee.ABSOLUTE(200),
            swap_id=swap_id,
            keys=claim_keys,
            chain_client=chain_client,
            boltz_api=boltz_api,
            options=kaleidoswap_sdk.TransactionOptions(
                chain_claim=kaleidoswap_sdk.ChainClaim(
                    keys=refund_keys, lockup_script=lockup_script
                )
            ),
        )

        claim_tx = await claim_script.construct_claim(preimage, claim_params)
        txid = await chain_client.broadcast_tx(claim_tx)
        print(f"Claim Transaction ID: {txid}")


async def main():
    await swap(btc_chain, lbtc_chain, False)
    await swap(lbtc_chain, btc_chain, False)
    await swap(btc_chain, lbtc_chain, True)
    await swap(lbtc_chain, btc_chain, True)


if __name__ == "__main__":
    asyncio.run(main())
