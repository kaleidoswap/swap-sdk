"""The high-level surface through generated bindings: the application
implements the store and the signer, the maker is a local HTTP stub. No chain
services are required; nothing here creates a swap, because that needs a
lockup tree the maker would have to sign."""

import asyncio
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import kaleidorg_swap_sdk as sdk

# A hold invoice the signet maker issued (50 000 sat), long since expired.
EXPIRED_INVOICE_SIGNET_50K = "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e"
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


MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"


class DeviceSigner(sdk.SwapSigner):
    """What an application provides: a signer whose keys stay where they
    are (a hardware wallet, an enclave). Stood in for here by the Rust
    default, reached only through the trait."""

    def __init__(self):
        self.device = sdk.MnemonicSigner(MNEMONIC, None, sdk.Network.SIGNET)
        self.requests = []

    def public_key(self, index):
        return self.device.public_key(index)

    def derive_preimage(self, index):
        return self.device.derive_preimage(index)

    def sign_schnorr(self, index, sighash):
        self.requests.append("sign_schnorr")
        return self.device.sign_schnorr(index, sighash)

    def musig_nonce(self, index, session):
        self.requests.append("musig_nonce")
        return self.device.musig_nonce(index, session)

    def musig_partial_sign(self, index, session, our_nonce, their_nonce):
        self.requests.append("musig_partial_sign")
        return self.device.musig_partial_sign(index, session, our_nonce, their_nonce)


def check_signer_encoding():
    """The hex the signer trait carries: 33-byte keys, 64-byte signatures,
    66-byte nonces, 32-byte partial signatures, and a nonce signs once."""
    ours = DeviceSigner()
    theirs = sdk.MnemonicSigner(MNEMONIC, "maker", sdk.Network.SIGNET)
    our_key, their_key = ours.public_key(0), theirs.public_key(0)
    assert len(bytes.fromhex(our_key)) == 33
    assert len(bytes.fromhex(ours.sign_schnorr(0, "11" * 32))) == 64
    session = sdk.MusigSession(
        pubkeys=[their_key, our_key], tweak="03" * 32, sighash="05" * 32
    )
    our_nonce = ours.musig_nonce(0, session)
    their_nonce = theirs.musig_nonce(0, session)
    assert len(bytes.fromhex(our_nonce)) == 66
    partial = ours.musig_partial_sign(0, session, our_nonce, their_nonce)
    assert len(bytes.fromhex(partial)) == 32
    try:
        ours.musig_partial_sign(0, session, our_nonce, their_nonce)
    except sdk.Error:
        pass
    else:
        raise AssertionError("a MuSig2 nonce signed twice")


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
    check_signer_encoding()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        url = f"http://127.0.0.1:{server.server_port}/v2"
        store = DictStore()
        kaleido = sdk.KaleidoSwap(options(store, DeviceSigner(), url))
        assert kaleido.network() == sdk.Network.SIGNET

        # A reverse quote prices from the pair card and binds its hash.
        answer = await kaleido.quote(
            sdk.QuoteRequest(
                pay=sdk.Asset.BTC_LIGHTNING(),
                receive=sdk.Asset.BTC_LIQUID(),
                amount=sdk.Amount.PAY(50_000),
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
                pay=sdk.Asset.BTC_LIGHTNING(),
                receive=sdk.Asset.BTC_LIQUID(),
                amount=sdk.Amount.PAY(1),
            )
        )
        assert below.quote is None
        assert below.refusal.reason == sdk.QuoteRefusalReason.AMOUNT_BELOW_MIN
        assert (below.refusal.limits.min, below.refusal.limits.max) == (
            10_000,
            1_000_000,
        )

        # A submarine is priced against its invoice, which has to be
        # unexpired: an expired one is refused before the maker is contacted.
        try:
            await kaleido.quote(
                sdk.QuoteRequest(
                    pay=sdk.Asset.BTC_ONCHAIN(),
                    receive=sdk.Asset.BTC_LIGHTNING(),
                    amount=sdk.Amount.INVOICE(EXPIRED_INVOICE_SIGNET_50K),
                )
            )
        except sdk.Error.Swap as error:
            assert code_of(error) == sdk.SwapErrorCode.INVOICE_EXPIRED, error
        else:
            raise AssertionError("an expired invoice was quoted")

        # A route this layer does not create swaps for is a refusal that says
        # where it lives; an RGB asset is named by its contract.
        rgb = await kaleido.quote(
            sdk.QuoteRequest(
                pay=sdk.Asset.BTC_LIGHTNING(),
                receive=sdk.Asset.RGB_ONCHAIN(contract_id="rgb:2dkSTbr-jFhznbPmo"),
                amount=sdk.Amount.PAY(50_000),
            )
        )
        assert rgb.quote is None
        assert rgb.refusal.reason == sdk.QuoteRefusalReason.PAIR_UNAVAILABLE
        assert "RGB swap" in rgb.refusal.message

        # Checks made before the maker is contacted come back coded.
        for start, expected in [
            (
                sdk.StartOptions(
                    destination="ex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j20sf6s5",
                    refund_address=None,
                ),
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
        # `start` reads the invoice itself rather than trusting the quote.
        stale = sdk.Quote(
            protocol=sdk.Protocol.BOLTZ_SUBMARINE,
            from_asset=sdk.Asset.BTC_ONCHAIN(),
            to_asset=sdk.Asset.BTC_LIGHTNING(),
            from_amount=50_200,
            to_amount=50_000,
            fees=quote.fees,
            limits=quote.limits,
            expires_at=quote.expires_at,
            binding=sdk.QuoteBinding(
                pair_hash="sub-hash",
                invoice=EXPIRED_INVOICE_SIGNET_50K,
                zero_conf_limit=0,
            ),
        )
        try:
            await kaleido.start(
                stale,
                sdk.StartOptions(
                    destination=None,
                    refund_address="tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
                ),
            )
        except sdk.Error.Swap as error:
            assert code_of(error) == sdk.SwapErrorCode.INVOICE_EXPIRED, error
        else:
            raise AssertionError("an expired invoice was started against")
        assert posts == [], "nothing reached the maker"

        # A record's string form must not be able to carry the maker's
        # per-swap credential: the whole document reaches a store through
        # `SwapStore.put`, never through the record type.
        import inspect

        record_fields = set(inspect.signature(sdk.SwapRecord.__init__).parameters)
        assert "json" not in record_fields, record_fields

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

        broken = sdk.KaleidoSwap(options(Broken(), DeviceSigner(), url))
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
                sdk.MnemonicSigner(MNEMONIC, None, sdk.Network.SIGNET),
                url,
            )
        )
        again = await defaults.quote(
            sdk.QuoteRequest(
                pay=sdk.Asset.BTC_LIGHTNING(),
                receive=sdk.Asset.BTC_LIQUID(),
                amount=sdk.Amount.PAY(50_000),
            )
        )
        assert again.quote.to_amount == 49_450
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


asyncio.run(run())
