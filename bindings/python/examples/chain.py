import kaleidoswap_sdk
import asyncio
from datetime import datetime

electrum_btc = kaleidoswap_sdk.ClientConnection.ELECTRUM(
    kaleidoswap_sdk.ElectrumBuilder(url="localhost:19001", tls=False)
)
electrum_lbtc = kaleidoswap_sdk.ClientConnection.ELECTRUM(
    kaleidoswap_sdk.ElectrumBuilder(url="localhost:19002", tls=False)
)
network = kaleidoswap_sdk.Network.REGTEST
chain_client = kaleidoswap_sdk.ChainClient(
    kaleidoswap_sdk.ClientConfig(
        network=network, bitcoin=electrum_btc, liquid=electrum_lbtc
    )
)


async def main():
    # Initialize the Boltz API client
    boltz_api = kaleidoswap_sdk.BoltzApiClientV2.default(network)

    btc_chain = kaleidoswap_sdk.btc_chain_from_network(network)
    lbtc_chain = kaleidoswap_sdk.lbtc_chain_from_network(network)

    from_chain = btc_chain
    to_chain = lbtc_chain

    # Initialize WebSocket client
    ws_client = boltz_api.ws()

    # Generate a new key pair for the swap
    claim_keys = kaleidoswap_sdk.KeyPair()
    refund_keys = kaleidoswap_sdk.KeyPair()

    # Get the amount to swap from user
    amount = int(input("Enter amount in sats to swap: "))
    claim_address = input(
        f"Enter claim address for {'liquid' if to_chain.is_liquid() else 'bitcoin'}: "
    )

    # Generate a preimage for the swap
    preimage = kaleidoswap_sdk.Preimage()

    # Create a chain swap request
    request = kaleidoswap_sdk.CreateChainRequest(
        _from=from_chain,
        to=to_chain,
        preimage_hash=preimage.sha256(),
        claim_public_key=claim_keys.public(),
        refund_public_key=refund_keys.public(),
        user_lock_amount=amount,
    )

    print("\n=== Creating Chain Swap ===")
    response = await boltz_api.create_chain_swap(request)
    swap_id = response.id
    print(f"Swap ID: {swap_id}")
    print(f"Lockup Address: {response.lockup_details.lockup_address}")
    print(f"Claim Address: {claim_address}")

    print("\n=== Instructions ===")
    print(
        f"1. Send {amount} sats to the lockup address: {response.lockup_details.lockup_address}"
    )
    print("2. Wait for the swap to be confirmed")
    print("3. The funds will be sent to the claim address automatically")
    print("\nMonitoring swap status via WebSocket...")

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
    updates = ws_client.updates()
    await ws_client.subscribe_swap(swap_id)
    while True:
        update = await updates.next()
        status = update.status

        print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Swap Status: {status}")

        if status == "swap.created":
            print("\n=== Action Required ===")
            print(
                f"Please send {amount} sats to: {response.lockup_details.lockup_address}"
            )
            print("Waiting for your transaction...")

        elif status == "transaction.mempool":
            print("Transaction detected in mempool!")

        elif status == "transaction.confirmed":
            print("Transaction confirmed!")

        elif status == "transaction.server.mempool":
            print("Server transaction detected in mempool!")

        elif status == "transaction.server.confirmed":
            print("Server transaction confirmed!")

            # Construct and broadcast claim transaction
            print("\n=== Constructing Claim Transaction ===")

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
            print("Claim transaction constructed, broadcasting...")
            await chain_client.broadcast_tx(claim_tx)
            print("Claim transaction broadcast!")

        elif status == "transaction.claimed":
            print("\n=== Success! ===")
            print("Swap completed successfully!")
            print(f"Funds have been sent to {claim_address}")
            break

        elif status in ["transaction.lockupFailed", "transaction.failed"]:
            print("\n=== Swap Failed ===")
            print("The swap could not be completed")

            # Construct and broadcast refund transaction
            print("\n=== Constructing Refund Transaction ===")

            refund_address = input("Enter refund address: ")

            refund_params = kaleidoswap_sdk.SwapTransactionParams(
                output_address=refund_address,
                fee=kaleidoswap_sdk.Fee.ABSOLUTE(200),
                swap_id=swap_id,
                keys=refund_keys,
                chain_client=chain_client,
                boltz_api=boltz_api,
            )

            refund_tx = await lockup_script.construct_refund(refund_params)
            print("Refund transaction constructed, broadcasting...")
            await refund_params.chain_client.broadcast_tx(refund_tx)
            print("Refund transaction broadcast!")
            break

        elif status == "swap.expired":
            print("\n=== Swap Expired ===")
            print("The swap has expired")
            break


if __name__ == "__main__":
    asyncio.run(main())
