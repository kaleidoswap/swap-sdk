"""Quote the Arkade Intents corridor in both directions, against the live signet maker.

`arkade:BTC <-> lightning:BTC` is not a Boltz-shaped route: the maker serves it
as an RFQ over `POST /v1/swap`, beside its `/v2` surface, and answers with a
binding quote or a refusal. This example asks for one quote per direction and
runs the checks a client owes BEFORE it commits value — nothing here funds a
lockup or pays an invoice.

What it deliberately cannot do is finish a swap. Funding the send lockup, or
claiming the receive lockup once the maker funds it, needs an Ark wallet; that
is `@kaleidorg/swap-sdk/arkade`'s job (TypeScript). This half — quote, verify,
track — is what a Python host can carry without that dependency.

A quote is a real reservation on the maker's side, so the receive request below
is left with an undecodable payout address on purpose: the maker refuses it
before creating anything. Replace `PAYOUT_ADDRESS` with your own Ark address to
receive a real quote — and then either pay its invoice or let it expire.
"""

import asyncio
import hashlib
import os
import time

import kaleidorg_swap_sdk as sdk

# secp256k1's generator point, x-only: a valid key nobody holds the secret to.
# Fine for a quote you will not act on; use your own wallet's key otherwise.
G_X = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

# Not an Ark address, so the maker refuses before it reserves anything.
PAYOUT_ADDRESS = os.environ.get("ARK_PAYOUT_ADDRESS", "not-an-ark-address")


def describe(answer: sdk.RfqAnswer) -> None:
    if answer.refusal is not None:
        # A refusal is the maker's answer, not an error: it is priced, deliberate,
        # and arrives as a value. `UNKNOWN` is a reason this SDK version has not
        # heard of — still a refusal.
        print(f"  refused: {answer.refusal.reason}")
        return
    quote = answer.quote
    assert quote is not None
    print(
        f"  quote {quote.rfq_id[:12]}…  give {quote.from_amount} sats, receive {quote.to_amount} sats"
    )
    print(
        f"  fee {quote.from_amount - quote.to_amount} sats, valid until {quote.valid_until}, refund deadline {quote.refund_locktime}"
    )
    print(f"  lockup {quote.profile.lockup_address}")


async def main() -> None:
    client = sdk.BoltzApiClientV2.default(sdk.Network.SIGNET)
    print("corridor:", client.corridor_url())

    # ---- receive: pay a hold invoice over Lightning, receive on Arkade ---------
    preimage = os.urandom(32)  # YOURS. The maker never sees it until you claim.
    payment_hash = hashlib.sha256(preimage).hexdigest()
    print("\nlightning:BTC->arkade:BTC")
    receive = await client.quote_lightning_receive(
        sdk.LightningReceiveRequest(
            rfq_id=sdk.new_rfq_id(),
            # "receive exactly this on Arkade": the maker inverts it through its
            # rate card, so the price is the free variable and rounds UP by a sat
            # or two — check `to_amount >= amount`, never equality.
            amount_side=sdk.AmountSide.TO,
            amount=12_000,
            payment_hash=payment_hash,
            payout_address=PAYOUT_ADDRESS,
            payout_pubkey=G_X,
            claim_packet=None,
        )
    )
    describe(receive)
    if receive.quote is not None:
        # Before the invoice reaches a payer: it is the MAKER's invoice, so check
        # that it pays OUR hash for exactly `from_amount`, then that a payment at
        # the deadline still leaves 30 minutes to claim before the maker's refund
        # opens. Both checks live on the quote in the Rust core; the Python
        # surface exposes the quote's fields, so here they are spelled out.
        assert receive.quote.profile.invoice, "a receive quote must name the invoice"
        assert receive.quote.to_amount >= 12_000
        now = int(time.time())
        assert receive.quote.valid_until > now, "quote already expired"
        print(
            f"  pay {receive.quote.from_amount} sats via: {receive.quote.profile.invoice[:40]}…"
        )

    # ---- send: fund an Arkade lockup, have a Lightning invoice paid -----------
    # A send needs a BOLT11 to pay and an Ark refund address to pin into the
    # covenant. Neither can be faked without creating a real reservation, so
    # this shows the request shape and the refusal it earns with a placeholder
    # invoice — the maker cannot decode "lnbc1...", and says so.
    print("\narkade:BTC->lightning:BTC")
    send = await client.quote_lightning_send(
        sdk.LightningSendRequest(
            rfq_id=sdk.new_rfq_id(),
            invoice=os.environ.get("BOLT11", "lnbc1..."),
            refund_address=os.environ.get("ARK_REFUND_ADDRESS", "not-an-ark-address"),
            client_refund_pubkey=G_X,
        )
    )
    describe(send)
    if send.quote is not None:
        # Before funding: the quote must still be valid and leave 90 minutes
        # before its refund deadline (median-time-past lags the clock by up to an
        # hour). Then derive the covenant from the binding fields with the
        # `./arkade` venue and refuse to fund on any address mismatch.
        now = int(time.time())
        assert send.quote.refund_locktime is not None
        assert send.quote.refund_locktime - now >= 90 * 60, "not enough refund headroom"
        print(
            f"  fund {send.quote.from_amount} sats at {send.quote.profile.lockup_address}"
        )

    # ---- track: poll by rfq_id until a terminal state ---------------------------
    for answer in (receive, send):
        if answer.quote is None:
            continue
        status = await client.rfq_status(answer.quote.rfq_id)
        print(
            f"\nstatus {answer.quote.rfq_id[:12]}…: {status.state if status else 'unknown id'}"
        )

    # An id the maker never issued is `None`, not an error.
    assert await client.rfq_status("0" * 64) is None


if __name__ == "__main__":
    asyncio.run(main())
