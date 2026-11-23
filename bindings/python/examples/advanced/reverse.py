import kaleidoswap
import asyncio
from datetime import datetime

electrum_btc = kaleidoswap.ClientConnection.ELECTRUM(
    kaleidoswap.ElectrumBuilder(url="localhost:19001", tls=False)
)
electrum_lbtc = kaleidoswap.ClientConnection.ELECTRUM(
    kaleidoswap.ElectrumBuilder(url="localhost:19002", tls=False)
)
network = kaleidoswap.Network.REGTEST
chain_client = kaleidoswap.ChainClient(
    kaleidoswap.ClientConfig(
        network=network, bitcoin=electrum_btc, liquid=electrum_lbtc
    )
)


async def main():
    # Initialize the Boltz API client
    network = kaleidoswap.Network.REGTEST
    boltz_api = kaleidoswap.SwapClient.default(network)

    to_chain = kaleidoswap.btc_chain_from_network(network)

    # Initialize WebSocket client
    ws_client = boltz_api.ws()

    # Generate a new key pair for the swap
    key_pair = kaleidoswap.KeyPair()

    # Get the amount to swap from user
    amount = int(input("Enter amount in sats to swap: "))

    claim_address = input(
        f"Enter claim address for {'liquid' if to_chain.is_liquid() else 'bitcoin'}: "
    )

    preimage = kaleidoswap.Preimage()

    # Create a reverse swap request
    request = kaleidoswap.CreateReverseRequest(
        invoice_amount=amount,
        _from=to_chain,
        to=to_chain,
        preimage_hash=preimage.sha256(),
        claim_public_key=key_pair.public(),
    )

    print("\n=== Creating Reverse Swap ===")
    response = await boltz_api.create_reverse_swap(request)
    swap_id = response.id
    print(f"Swap ID: {swap_id}")
    print(f"Lockup Address: {response.lockup_address}")
    print(f"Lightning Invoice: {response.invoice}")

    print("\n=== Instructions ===")
    print("1. Pay the Lightning invoice above")
    print("2. Wait for the swap to be confirmed")
    print("3. The funds will be sent to your lockup address automatically")
    print("\nMonitoring swap status via WebSocket...")

    asyncio.create_task(ws_client.run_ws_loop())

    # Monitor the swap status via WebSocket
    updates = ws_client.updates()
    await ws_client.subscribe_swap(swap_id)
    while True:
        update = await updates.next()
        status = update.status

        print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Swap Status: {status}")

        if status == "swap.created":
            print("\n=== Action Required ===")
            print(f"Please pay the Lightning invoice: {response.invoice}")
            print("Waiting for your payment...")

        elif status == "transaction.mempool":
            print("Transaction detected in mempool!")
            print("Waiting for transaction to be confirmed... (mine a block)")

        elif status == "transaction.confirmed":
            print("Lockup transaction confirmed!")

            swap_script = kaleidoswap.SwapScript.from_reverse(
                chain=to_chain, reverse_response=response, our_pubkey=key_pair.public()
            )

            def claim_params(cooperative):
                return kaleidoswap.SwapTransactionParams(
                    swap_id=swap_id,
                    keys=key_pair,
                    fee=kaleidoswap.Fee.ABSOLUTE(200),
                    output_address=claim_address,
                    chain_client=chain_client,
                    boltz_api=boltz_api,
                    options=kaleidoswap.TransactionOptions(cooperative=cooperative),
                )

            # Cooperative first. The script path needs nothing from the maker,
            # so any failure of the cooperative claim falls back to it.
            try:
                tx = await swap_script.construct_claim(preimage, claim_params(True))
            except Exception as error:
                print(
                    f"Cooperative claim unavailable ({error}); claiming by script path"
                )
                tx = await swap_script.construct_claim(preimage, claim_params(False))

            print("Transaction signed, broadcasting...")
            tx_id = await chain_client.broadcast_tx(tx)
            print(f"Transaction ID: {tx_id}")

        elif status == "invoice.settled":
            print("\n=== Success! ===")
            print("Swap completed successfully!")
            print(f"Funds have been sent to {claim_address}")
            break

        elif status in ["transaction.lockupFailed", "invoice.failedToPay"]:
            print("\n=== Swap Failed ===")
            print("The swap could not be completed")
            break

        elif status == "expired":
            print("\n=== Swap Expired ===")
            print("The swap has expired")
            break


if __name__ == "__main__":
    asyncio.run(main())
