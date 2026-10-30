"""The high-level surface through generated bindings: the application
implements the store and the signer, the maker is a local HTTP stub. No chain
services are required; nothing here creates a swap, because that needs a
lockup tree the maker would have to sign."""

import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import kaleidorg_swap_sdk as sdk

INVOICE_SIGNET_50K = "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e"
REVERSE_CARD = {
    "hash": "rev-hash",
    "rate": 1,
    "limits": {"minimal": 10000, "maximal": 1000000},
    "fees": {"percentage": 0.5, "minerFees": {"lockup": 200, "claim": 100}},
}
SUBMARINE_CARD = {
    "hash": "sub-hash",
    "rate": 1,
    "limits": {"minimal": 10000, "maximal": 1000000, "maximalZeroConf": 0},
    "fees": {"percentage": 0.1, "minerFees": 150},
}
posts = []


class Maker(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.endswith("/v2/swap/reverse"):
            body = {"BTC": {"L-BTC": REVERSE_CARD, "BTC": REVERSE_CARD}}
        elif self.path.endswith("/v2/swap/submarine"):
            body = {"BTC": {"BTC": SUBMARINE_CARD}, "L-BTC": {"BTC": SUBMARINE_CARD}}
        else:
            self.send_response(404)
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(body).encode())

    def do_POST(self):
        posts.append(self.path)
        self.send_response(500)
        self.end_headers()

    def log_message(self, *_args):
        pass


class DictStore(sdk.SwapStore):
    """What an application provides: a database, here a dict."""

    def __init__(self):
        self.records = {}
        self.index = 0

    def get(self, id):
        return self.records.get(id)

    def put(self, id, record_json):
        self.records[id] = record_json

    def list(self):
        return list(self.records.values())

    def next_index(self):
        index, self.index = self.index, self.index + 1
        return index


class FixedSigner(sdk.SwapSigner):
    def derive_key(self, index):
        return f"{index + 1:064x}"

    def derive_preimage(self, index):
        return f"{index + 1:064x}"


def options(store, signer, url):
    return sdk.KaleidoSwapOptions(
        network=sdk.Network.SIGNET,
        store=store,
        signer=signer,
        maker_url=url,
        bitcoin_esplora_url="http://127.0.0.1:1",
        liquid_esplora_url="http://127.0.0.1:1",
        poll_interval_ms=1,
    )


def code_of(error):
    assert isinstance(error, sdk.Error.Swap), error
    return error.code


async def run():
    server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        url = f"http://127.0.0.1:{server.server_port}/v2"
        store = DictStore()
        kaleido = sdk.KaleidoSwap(options(store, FixedSigner(), url))
        assert kaleido.network() == sdk.Network.SIGNET

        # A reverse quote prices from the pair card and binds its hash.
        answer = await kaleido.quote(
            sdk.QuoteRequest(
                from_asset=sdk.AssetId.LIGHTNING_BTC,
                to_asset=sdk.AssetId.LIQUID_BTC,
                side=None,
                amount=50_000,
                invoice=None,
            )
        )
        assert answer.refusal is None
        quote = answer.quote
        assert quote.protocol == sdk.Protocol.BOLTZ_REVERSE
        assert (quote.from_amount, quote.to_amount) == (50_000, 49_450)
        assert (quote.fees.maker, quote.fees.miner) == (250, 300)
        assert quote.binding.pair_hash == "rev-hash"

        # A refusal is a value, with the limits it came from.
        below = await kaleido.quote(
            sdk.QuoteRequest(
                from_asset=sdk.AssetId.LIGHTNING_BTC,
                to_asset=sdk.AssetId.LIQUID_BTC,
                side=None,
                amount=1,
                invoice=None,
            )
        )
        assert below.quote is None
        assert below.refusal.reason == sdk.QuoteRefusalReason.AMOUNT_BELOW_MIN
        assert (below.refusal.limits.min, below.refusal.limits.max) == (
            10_000,
            1_000_000,
        )

        # A submarine is priced against its invoice.
        sub = await kaleido.quote(
            sdk.QuoteRequest(
                from_asset=sdk.AssetId.BITCOIN_BTC,
                to_asset=sdk.AssetId.LIGHTNING_BTC,
                side=None,
                amount=None,
                invoice=INVOICE_SIGNET_50K,
            )
        )
        assert sub.quote.protocol == sdk.Protocol.BOLTZ_SUBMARINE
        assert (sub.quote.to_amount, sub.quote.from_amount) == (50_000, 50_200)

        # Checks made before the maker is contacted come back coded.
        for start, expected in [
            (
                sdk.StartOptions(destination="lq1mainnet", refund_address=None),
                sdk.SwapErrorCode.WRONG_NETWORK_ADDRESS,
            ),
            (
                sdk.StartOptions(destination=None, refund_address=None),
                sdk.SwapErrorCode.INVALID_DESTINATION,
            ),
        ]:
            try:
                await kaleido.start(quote, start)
            except sdk.Error.Swap as error:
                assert code_of(error) == expected, error
            else:
                raise AssertionError("an unusable destination was accepted")
        try:
            await kaleido.start(
                sub.quote, sdk.StartOptions(destination=None, refund_address=None)
            )
        except sdk.Error.Swap as error:
            assert code_of(error) == sdk.SwapErrorCode.REFUND_ADDRESS_REQUIRED
        else:
            raise AssertionError("a submarine without a refund address was accepted")
        assert posts == [], "nothing reached the maker"

        # Unknown ids, and an empty store, behave.
        try:
            await kaleido.swap("nope")
        except sdk.Error.Swap as error:
            assert code_of(error) == sdk.SwapErrorCode.SWAP_NOT_FOUND
        report = await kaleido.resume()
        assert (
            report.resumed,
            report.settled,
            report.refunded,
            report.failed,
            report.errors,
        ) == ([], [], [], [], [])

        # A store that fails is reported as such, not swallowed.
        class Broken(DictStore):
            def list(self):
                raise sdk.Error.Generic("disk gone")

        broken = sdk.KaleidoSwap(options(Broken(), FixedSigner(), url))
        try:
            await broken.resume()
        except sdk.Error.Swap as error:
            assert code_of(error) == sdk.SwapErrorCode.STORE_FAILED
        else:
            raise AssertionError("a failing store was not reported")

        # The Rust defaults implement the same traits.
        defaults = sdk.KaleidoSwap(
            options(
                sdk.MemoryStore(),
                sdk.MnemonicSigner(
                    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                    None,
                    sdk.Network.SIGNET,
                ),
                url,
            )
        )
        again = await defaults.quote(
            sdk.QuoteRequest(
                from_asset=sdk.AssetId.LIGHTNING_BTC,
                to_asset=sdk.AssetId.LIQUID_BTC,
                side=None,
                amount=50_000,
                invoice=None,
            )
        )
        assert again.quote.to_amount == 49_450
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


asyncio.run(run())
