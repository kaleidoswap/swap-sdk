"""Offline L-USDT binding coverage: the asset pin against a local HTTP maker.

The maker serves pair cards and create responses from the frozen L-USDT wire
contract. The pin, not the card, decides which asset is L-USDT.
"""

import asyncio
import copy
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import kaleidorg_swap_sdk as sdk

ROOT = Path(__file__).resolve().parents[3]
WIRE = json.loads((ROOT / "tests/fixtures/lusdt-v1/wire-contract.json").read_text())
LUSDT = WIRE["assets"]["lusdt"]
POLICY = WIRE["assets"]["policy"]
OTHER = "22" * 32
BTC = sdk.Chain.BITCOIN(sdk.BitcoinChain.BITCOIN_REGTEST)
LIQUID = sdk.Chain.LIQUID(sdk.LiquidChain.LIQUID_REGTEST)
REQUESTS = []
PAIRS = copy.deepcopy(WIRE["pairResponses"])
REPLY = None


class Maker(BaseHTTPRequestHandler):
    def do_GET(self):
        kind = self.path.rsplit("/", 1)[-1]
        self._reply(PAIRS[kind])

    def do_POST(self):
        REQUESTS.append(
            json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        )
        self._reply(REPLY)

    def _reply(self, payload):
        body = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


async def refused(fn, message, posts=False):
    before = len(REQUESTS)
    try:
        await fn()
    except sdk.Error.Generic as error:
        assert message.lower() in str(error).lower(), str(error)
    else:
        raise AssertionError("expected error")
    if posts:
        assert len(REQUESTS) == before + 1
    else:
        assert len(REQUESTS) == before, "the refusal must not reach the maker"


async def main(url):
    global REPLY
    client = sdk.SwapClient(url, None)

    reverse = sdk.CreateReverseRequest(
        _from=BTC,
        to=LIQUID,
        from_currency=sdk.Currency.BTC,
        to_currency=sdk.Currency.L_USDT,
        preimage_hash=WIRE["create"]["reverse"]["request"]["preimageHash"],
        claim_public_key=WIRE["create"]["reverse"]["request"]["claimPublicKey"],
        invoice_amount=WIRE["create"]["reverse"]["request"]["invoiceAmount"],
    )
    submarine = sdk.CreateSubmarineRequest(
        _from=LIQUID,
        to=BTC,
        from_currency=sdk.Currency.L_USDT,
        to_currency=sdk.Currency.BTC,
        invoice=WIRE["create"]["submarine"]["request"]["invoice"],
        refund_public_key=WIRE["create"]["submarine"]["request"]["refundPublicKey"],
    )
    chain_request = WIRE["create"]["chain"]["userAmountRequest"]
    chain = sdk.CreateChainRequest(
        _from=BTC,
        to=LIQUID,
        from_currency=sdk.Currency.BTC,
        to_currency=sdk.Currency.L_USDT,
        preimage_hash=chain_request["preimageHash"],
        claim_public_key=chain_request["claimPublicKey"],
        refund_public_key=chain_request["refundPublicKey"],
        user_lock_amount=chain_request.get("userLockAmount"),
    )

    for kind, request, create, card, asset_field in [
        (
            "reverse",
            reverse,
            client.create_reverse_swap,
            ("BTC", "L-USDT"),
            "toAssetId",
        ),
        (
            "submarine",
            submarine,
            client.create_swap,
            ("L-USDT", "BTC"),
            "fromAssetId",
        ),
        ("chain", chain, client.create_chain_swap, ("BTC", "L-USDT"), "toAssetId"),
    ]:
        REPLY = copy.deepcopy(WIRE["create"][kind]["response"])
        pair = PAIRS[kind][card[0]][card[1]]

        # Regtest has no canonical L-USDT: a missing pin is refused before
        # any request is posted, however good the pair card looks.
        for missing in [None, "", "   "]:
            request.lusdt_asset_id = missing
            await refused(lambda: create(request), "lusdt_asset_id")

        # A pin the maker disagrees with, or a maker that advertises another
        # asset as L-USDT, is refused before the swap exists.
        request.lusdt_asset_id = OTHER
        await refused(lambda: create(request), "pair card")
        request.lusdt_asset_id = POLICY
        await refused(lambda: create(request), "L-BTC")
        request.lusdt_asset_id = LUSDT
        pair[asset_field] = OTHER
        await refused(lambda: create(request), "pair card")
        pair[asset_field] = LUSDT

        # The pin is local: the maker never sees it.
        response = await create(request)
        assert "lusdtAssetId" not in REQUESTS[-1]
        assert "lusdt_asset_id" not in REQUESTS[-1]
        details = response.claim_details if kind == "chain" else response
        assert details.asset_id == LUSDT

        # A response locking another asset fails validation after the post.
        if kind == "chain":
            REPLY["claimDetails"]["assetId"] = OTHER
        else:
            REPLY["assetId"] = OTHER
        await refused(lambda: create(request), "currency", posts=True)


server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    asyncio.run(main(f"http://127.0.0.1:{server.server_port}/v2"))
finally:
    server.shutdown()
    server.server_close()
    thread.join()
