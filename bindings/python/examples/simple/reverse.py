"""Reverse swap with the high-level surface: a Lightning payment -> L-BTC on Liquid.

You name what you pay and what you receive; the SDK prices the route, checks
the maker's response, persists the swap, and claims the L-BTC to your address
once the maker's lockup confirms. The engine-level version, against
regtest BTC, is ../advanced/reverse.py.

    KALEIDO_SWAP_MNEMONIC="…12 words…" LIQUID_CLAIM_ADDRESS="tlq1…" \\
    python examples/simple/reverse.py
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
            pay=sdk.Asset.BTC_LIGHTNING(),
            receive=sdk.Asset.BTC_LIQUID(),
            # What you pay; sdk.Amount.RECEIVE(...) fixes what lands instead.
            amount=sdk.Amount.PAY(50_000),
        )
    )
    if answer.refusal:
        raise SystemExit(f"{answer.refusal.reason}: {answer.refusal.message}")
    quote = answer.quote
    print(
        f"pay {quote.from_amount} sat over Lightning, receive {quote.to_amount} sat of L-BTC "
        f"(maker fee {quote.fees.maker}, miner fee {quote.fees.miner})"
    )

    swap = await kaleido.start(
        quote,
        sdk.StartOptions(
            destination=os.environ["LIQUID_CLAIM_ADDRESS"], refund_address=None
        ),
    )
    print(f"\nPay this invoice ({quote.from_amount} sat):\n\n{swap.action().invoice}\n")

    # watch() also ends on refunded and failed, as an error; only settled is a
    # success.
    try:
        await swap.watch(PrintPhases())
    except sdk.Error.Swap as error:
        raise SystemExit(f"swap {swap.id()} ended without settling: {error}")
    print(
        f"Done: ~{quote.to_amount} sat of L-BTC reached the claim address ({swap.id()})"
    )


if __name__ == "__main__":
    asyncio.run(main())
