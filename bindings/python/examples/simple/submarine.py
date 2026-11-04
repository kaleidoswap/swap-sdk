"""Submarine swap with the high-level surface: on-chain BTC -> a Lightning invoice.

You name what you pay and what you receive; the SDK prices the route, checks
the maker's response, persists the swap, and refunds on its own if the maker
cannot pay. The engine-level version is ../advanced/submarine.py.

    KALEIDO_SWAP_MNEMONIC="…12 words…" BOLT11_INVOICE="lntbs…" \\
    REFUND_ADDRESS="tb1…" python examples/simple/submarine.py
"""

import asyncio
import os

import kaleidorg_swap_sdk as sdk

NETWORK = sdk.Network.SIGNET
# The store keeps every swap until it ends: claim and refund material, and the
# maker's swap_auth. It is gitignored; treat it as secret.
STORE = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "swaps", "simple"
)


class PrintPhases(sdk.PhaseListener):
    def on_phase(self, phase):
        print(f"  {phase.name.lower()}")


async def main():
    os.makedirs(STORE, exist_ok=True)
    kaleido = sdk.KaleidoSwap(
        sdk.KaleidoSwapOptions(
            network=NETWORK,
            store=sdk.FileStore(STORE),
            signer=sdk.MnemonicSigner(
                os.environ["KALEIDO_SWAP_MNEMONIC"], None, NETWORK
            ),
        )
    )
    await kaleido.resume()  # claims or refunds anything a previous run left behind

    answer = await kaleido.quote(
        sdk.QuoteRequest(
            pay=sdk.Asset.BTC_ONCHAIN(),
            receive=sdk.Asset.BTC_LIGHTNING(),
            amount=sdk.Amount.INVOICE(os.environ["BOLT11_INVOICE"]),  # fixes the amount
        )
    )
    if answer.refusal:
        raise SystemExit(f"{answer.refusal.reason}: {answer.refusal.message}")
    quote = answer.quote
    print(
        f"{quote.from_amount} sat on chain pays a {quote.to_amount} sat invoice "
        f"(maker fee {quote.fees.maker}, miner fee {quote.fees.miner})"
    )

    swap = await kaleido.start(
        quote,
        sdk.StartOptions(destination=None, refund_address=os.environ["REFUND_ADDRESS"]),
    )
    action = swap.action()
    print(
        f"\nFund exactly {action.funding_amount} sat to:\n  {action.funding_address}\n"
    )

    # watch() also ends on refunded and failed, as an error; only settled is a
    # success.
    try:
        await swap.watch(PrintPhases())
    except sdk.Error.Swap as error:
        raise SystemExit(f"swap {swap.id()} ended without paying the invoice: {error}")
    print(f"Done: the invoice was paid ({swap.id()})")


if __name__ == "__main__":
    asyncio.run(main())
