"""Retain maker recovery/status metadata through generated native DTOs."""

import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import kaleidorg_swap_sdk as sdk

LARGE = 9007199254740993
ENTRY = {
    "id": "fixture",
    "type": "reverse",
    "status": "swap.created",
    "createdAt": LARGE,
    "from": "BTC",
    "to": "BTC",
    "preimageHash": "ab" * 32,
    "claimDetails": {
        "type": "utxo",
        "amount": LARGE,
        "keyIndex": 7,
        "lockupAddress": "bcrt1pfixture",
        "serverPublicKey": "02" * 33,
        "timeoutBlockHeight": 144,
        "preimageHash": "ab" * 32,
        "tree": {
            "claimLeaf": {"output": "00", "version": 192},
            "refundLeaf": {"output": "00", "version": 192},
        },
    },
}
STATUS = {
    "status": "transaction.mempool",
    "events": [{"kind": "payment_attempted", "ts": LARGE, "data": {"attempt": LARGE}}],
    "payout": {
        "mode": "direct",
        "destination": "tb1pfixture",
        "layer": "bitcoin",
        "reference": "txid",
    },
}


class Maker(BaseHTTPRequestHandler):
    def respond(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        self.respond([ENTRY] if self.path == "/swap/restore" else {"index": -1})

    def do_GET(self):
        self.respond(STATUS)

    def log_message(self, *_args):
        pass


async def run(url):
    assert sdk.SwapEvent(kind="legacy_event", ts=1).data is None
    assert sdk.SwapPayoutStatus(mode="direct", destination="address").layer is None
    client = sdk.SwapClient(url, None)
    (restored,) = await client.swap_restore("xpub", "m", None)
    assert restored.created_at == LARGE
    assert restored.preimage_hash == ENTRY["preimageHash"]
    assert restored.claim_details.detail_type == "utxo"
    assert restored.claim_details.amount == LARGE
    assert restored.refund_details is None
    assert (await client.swap_restore_index("xpub", "m", None)).index == -1
    status = await client.get_swap("fixture")
    assert status.payout.reference == "txid"
    assert status.events[0].ts == LARGE
    assert json.loads(status.events[0].data)["attempt"] == LARGE


server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    asyncio.run(run(f"http://127.0.0.1:{server.server_port}"))
finally:
    server.shutdown()
    thread.join()
    server.server_close()
