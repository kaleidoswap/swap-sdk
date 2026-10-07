"""Offline RGB binding coverage with synthetic PSBTs and a local HTTP maker.

No rgb-lib proofs are fabricated here: these test the SDK/wallet boundary.
"""

import asyncio
import copy
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import kaleidorg_swap_sdk as sdk

VECTORS = json.loads(
    (Path(__file__).resolve().parent.parent / "fixtures/rgb-spends.json").read_text()
)
CHAIN = sdk.Chain.BITCOIN(sdk.BitcoinChain.BITCOIN_REGTEST)
KEYS = sdk.KeyPair.from_secret_key(VECTORS["secretKeyHex"])
PREIMAGE = sdk.Preimage.from_bytes(bytes.fromhex(VECTORS["preimageHex"]))
REQUESTS = []
REQUEST_METADATA = []
REPLY = None


class Maker(BaseHTTPRequestHandler):
    def do_POST(self):
        REQUEST_METADATA.append((self.path, self.headers.get("X-Swap-Auth")))
        REQUESTS.append(
            json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        )
        body = json.dumps(REPLY).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


def expect_error(fn, message, error_type=sdk.Error.Generic):
    try:
        fn()
    except error_type as error:
        assert message.lower() in str(error).lower(), str(error)
    else:
        raise AssertionError("expected error")


async def expect_async_error(fn, message):
    try:
        await fn()
    except sdk.Error.Generic as error:
        assert message.lower() in str(error).lower(), str(error)
    else:
        raise AssertionError("expected error")


async def main(url):
    global REPLY
    client = sdk.SwapClient(url, None)
    chain_client = sdk.ChainClient(
        sdk.ClientConfig(
            network=sdk.Network.REGTEST,
            bitcoin=sdk.ClientConnection.ESPLORA(sdk.EsploraBuilder(url=url)),
            liquid=None,
        )
    )
    for reverse in [True, False]:
        vector = VECTORS["swaps"]["reverse" if reverse else "submarine"]
        if reverse:
            request = sdk.CreateReverseRequest(
                _from=CHAIN,
                to=CHAIN,
                from_currency=sdk.Currency.BTC,
                to_currency=sdk.Currency.USDT_RGB,
                preimage_hash=PREIMAGE.sha256(),
                claim_public_key=KEYS.public(),
                invoice_amount=1005,
            )
            create = client.create_reverse_swap
        else:
            request = sdk.CreateSubmarineRequest(
                _from=CHAIN,
                to=CHAIN,
                from_currency=sdk.Currency.USDT_RGB,
                to_currency=sdk.Currency.BTC,
                invoice=VECTORS["invoice"],
                refund_public_key=KEYS.public(),
            )
            create = client.create_swap
        for missing in [None, "", "   "]:
            request.rgb_contract_id = missing
            before = len(REQUESTS)
            await expect_async_error(lambda: create(request), "rgb_contract_id")
            assert len(REQUESTS) == before, "a missing pin must not reach the maker"
        request.rgb_contract_id = VECTORS["contractId"]
        REPLY = copy.deepcopy(vector["response"])
        response = await create(request)
        assert response.rgb.asset_id == VECTORS["contractId"]
        assert "rgbContractId" not in REQUESTS[-1]
        assert "rgb_contract_id" not in REQUESTS[-1]
        if not reverse:
            REPLY["rgb"]["htlcSat"] = 5000000
            await expect_async_error(lambda: create(request), "collateral cap")
            request.rgb_max_htlc_sat = 5000000
            approved = await create(request)
            assert approved.rgb.htlc_sat == 5000000
            assert "rgbMaxHtlcSat" not in REQUESTS[-1]
            request.rgb_max_htlc_sat = None
            REPLY = copy.deepcopy(vector["response"])
        REPLY["rgb"]["assetId"] = "rgb:substituted-contract"
        await expect_async_error(lambda: create(request), "contract")

        script = (
            sdk.SwapScript.from_reverse(CHAIN, response, KEYS.public())
            if reverse
            else sdk.SwapScript.from_submarine(CHAIN, response, KEYS.public())
        )
        params = sdk.RgbPsbtParams(
            output_address=VECTORS["payoutAddress"],
            funding=(
                sdk.RgbSpendFunding.HTLC_VALUE(fee_rate_sat_vb=5)
                if reverse
                else sdk.RgbSpendFunding.CALLER_INPUTS()
            ),
            max_fee=10000,
            swap_id=response.id,
            chain_client=chain_client,
            boltz_api=client,
            lockup_tx=sdk.BtcLikeTransaction.from_hex_bitcoin(vector["lockTxHex"]),
        )
        pinned_lock = params.lockup_tx
        params.lockup_tx = None
        try:
            await (
                script.prepare_rgb_claim(params)
                if reverse
                else script.prepare_rgb_refund(params)
            )
        except (TypeError, AttributeError):
            pass
        else:
            raise AssertionError("RGB lock transaction must be required")
        params.lockup_tx = pinned_lock
        prepare = script.prepare_rgb_claim if reverse else script.prepare_rgb_refund
        if not reverse:
            await expect_async_error(
                lambda: script.prepare_rgb_cooperative_refund(params), "advertise"
            )
        spend = await prepare(params)
        template = spend.template()
        assert template.amount == 1005
        assert template.commitment_output_index == 0
        assert template.payment_output_index == 1
        assert template.payment_value == (546 if reverse else 1000)
        if not reverse:
            funded = spend.fund(vector["fundedPsbt"])
            assert spend.template().requires_funding, (
                "fund must leave the original intact"
            )
            spend = funded
            assert not spend.template().requires_funding
            assert spend.template().swap_input_index == 1
            assert spend.template().psbt == vector["fundedPsbt"]
        colored = sdk.ColoredRgbPsbt(
            psbt=vector["coloredPsbt"],
            allocations=[
                sdk.RgbAllocation(asset_id=VECTORS["contractId"], vout=1, amount=1005)
            ],
        )
        finalize = (
            lambda: spend.finalize_claim(colored, KEYS, PREIMAGE)
            if reverse
            else spend.finalize_refund(colored, KEYS)
        )
        colored.allocations[0].amount = 1004
        expect_error(finalize, "allocation")
        colored.allocations[0].amount = 1005
        finalized = finalize()
        assert finalized.psbt != colored.psbt, "the HTLC witness must be added"
        assert finalized.swap_input_index == (0 if reverse else 1)
        if reverse:
            assert finalized.transaction is not None
            assert len(finalized.transaction.txid()) == 64
            assert PREIMAGE.to_string() in finalized.transaction.hex()
        else:
            assert finalized.transaction is None, (
                "the wallet fee input remains unsigned"
            )
            params.funding = sdk.RgbSpendFunding.HTLC_VALUE(fee_rate_sat_vb=5)
            try:
                await script.prepare_rgb_refund(params)
            except sdk.Error.RgbFeeInputRequired:
                pass
            else:
                raise AssertionError("insufficient HTLC sats need the typed fee error")

        tx_params = sdk.SwapTransactionParams(
            output_address=VECTORS["payoutAddress"],
            fee=sdk.Fee.ABSOLUTE(100),
            swap_id=response.id,
            keys=KEYS,
            chain_client=chain_client,
            boltz_api=client,
        )
        await expect_async_error(lambda: script.construct_refund(tx_params), "RGB")

    chain_request = sdk.CreateChainRequest(
        _from=CHAIN,
        to=CHAIN,
        from_currency=sdk.Currency.USDT_RGB,
        to_currency=sdk.Currency.BTC,
        preimage_hash=PREIMAGE.sha256(),
        claim_public_key=KEYS.public(),
        refund_public_key=KEYS.public(),
    )
    before = len(REQUESTS)
    await expect_async_error(
        lambda: client.create_chain_swap(chain_request), "unsupported"
    )
    assert len(REQUESTS) == before
    coop_request = sdk.RgbCooperativeRefundRequest(
        protocol="rgb-coop-refund-v1",
        psbt="colored-psbt",
        index=1,
        pub_nonce="55" * 66,
        session_id="11" * 32,
    )
    await expect_async_error(
        lambda: client.get_rgb_refund_partial_sig("swap-id", coop_request, ""),
        "swapAuth",
    )
    assert len(REQUESTS) == before
    REPLY = dict(
        sessionId="11" * 32,
        requestHash="22" * 32,
        pubNonce="33" * 66,
        partialSignature="44" * 32,
    )
    reply = await client.get_rgb_refund_partial_sig(
        "swap-id", coop_request, "test-credential"
    )
    assert reply.session_id == coop_request.session_id
    assert REQUEST_METADATA[-1] == (
        "/v2/swap/submarine/swap-id/refund",
        "test-credential",
    )
    assert REQUESTS[-1] == dict(
        protocol=coop_request.protocol,
        psbt=coop_request.psbt,
        index=1,
        pubNonce=coop_request.pub_nonce,
        sessionId=coop_request.session_id,
    )


server = ThreadingHTTPServer(("127.0.0.1", 0), Maker)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    asyncio.run(main(f"http://127.0.0.1:{server.server_port}/v2"))
finally:
    server.shutdown()
    server.server_close()
    thread.join()
