"""Discovery through generated bindings; no chain services required."""

import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import kaleidorg_swap_sdk as sdk

card = json.loads(
    (
        Path(__file__).resolve().parents[3] / "tests/fixtures/corridor-v1/card.json"
    ).read_text()
)
requests = []
status = 200
body = json.dumps({**card, "future_field": "preserved"})


class Maker(BaseHTTPRequestHandler):
    def do_GET(self):
        requests.append(self.path)
        self.send_response(status)
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, *_args):
        pass


async def run():
    global status, body
    server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        client = sdk.SwapClient(
            f"http://127.0.0.1:{server.server_port}/prefix/v2/", None
        )
        result = await client.corridor_card()
        assert result.name == card["name"]
        assert result.transports.nostr.relays == card["transports"]["nostr"]["relays"]
        assert result.markets[0].max_quote_amount == "9007199254740993"
        assert result.sig == card["sig"]
        assert await client.corridor_card_raw() == body
        status, body = 200, json.dumps({**card, "version": 1})
        for fetch in [client.corridor_card, client.corridor_card_raw]:
            try:
                await fetch()
            except sdk.Error as error:
                assert "unsupported corridor card version" in str(error)
            else:
                raise AssertionError("unsupported schema version accepted")
        assert "Unverified" in sdk.CorridorCard.__doc__
        status, body = 404, "no live card"
        assert await client.corridor_card() is None
        for status, body in [(503, "unavailable"), (200, "{}"), (200, "invalid JSON")]:
            try:
                await client.corridor_card()
            except sdk.Error:
                pass
            else:
                raise AssertionError(
                    "a failed or malformed response must remain an error"
                )
        assert requests == ["/prefix/v1/card"] * 8
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


asyncio.run(run())
