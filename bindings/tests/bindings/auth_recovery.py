"""Signed credential recovery through the generated UniFFI surface."""

import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import kaleidorg_swap_sdk as sdk

SWAP_ID = "01KZZYB138E7C3HZX7Q1YBGAQG"
AUTH = "a1" * 32
CHALLENGE = (1800000300).to_bytes(8, "big") + bytes([42]) * 64
requests = []


class Maker(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append((self.path, dict(self.headers), body))
        if self.path.endswith("/auth/challenge"):
            response = {"challenge": CHALLENGE.hex(), "expiresAt": 1800000300}
        elif self.path.endswith("/auth/recover"):
            response = {"swapAuth": AUTH}
        else:
            response = {}
        data = json.dumps(response).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args):
        pass


async def run():
    server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        client = sdk.SwapClient(f"http://127.0.0.1:{server.server_port}/v2", None)
        keys = sdk.KeyPair()
        auth = await client.recover_swap_auth(SWAP_ID, keys)
        assert auth == AUTH
        await client.accept_quote(SWAP_ID, 93500, auth)
        assert [r[0] for r in requests] == [
            f"/v2/swap/{SWAP_ID}/auth/challenge",
            f"/v2/swap/{SWAP_ID}/auth/recover",
            f"/v2/swap/chain/{SWAP_ID}/quote",
        ]
        assert requests[1][2]["challenge"] == CHALLENGE.hex()
        assert len(bytes.fromhex(requests[1][2]["signature"])) == 64
        assert {k.lower(): v for k, v in requests[2][1].items()}["x-swap-auth"] == auth
        assert sdk.TransactionOptions(swap_auth=auth).swap_auth == auth
        assert sdk.TransactionOptions().swap_auth is None
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


asyncio.run(run())
