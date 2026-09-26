# RGB (USDT-RGB on Bitcoin L1) from the UniFFI surface: the taker's colored
# claim of a reverse lock, end to end on a fixture generated from the Rust SDK's
# own test helpers.
#
# Deliberately does not `import common`: nothing here needs the regtest
# environment or the network.

import base64

import kaleidorg_swap_sdk as sdk

TAKER_SECRET = "0101010101010101010101010101010101010101010101010101010101010101"
TAKER_PUBKEY = "031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"
PREIMAGE = bytes.fromhex(
    "0707070707070707070707070707070707070707070707070707070707070707"
)
DEST = "51200303030303030303030303030303030303030303030303030303030303030303"
LOCK_TX_HEX = "02000000010000000000000000000000000000000000000000000000000000000000000000ffffffff00fdffffff01f605000000000000225120a8bcb0e66bd1d2df008a6c504ba26a98a59c982dcd93cb35b191c2d7a53cd8c900000000"

LOCK = sdk.RgbLock(
    asset_id="rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd",
    amount=1000000,
    recipient_id="bcrt:wvout:htlc",
    blinding=42,
    htlc_sat=1526,
    claim_fee_rate=5,
    script_pubkey="5120a8bcb0e66bd1d2df008a6c504ba26a98a59c982dcd93cb35b191c2d7a53cd8c9",
    transport_endpoints=["rpcs://proxy.example/json-rpc"],
    min_confirmations=1,
)

RESPONSE = sdk.CreateReverseResponse(
    id="SWAP",
    invoice=None,
    swap_tree=sdk.SwapTree(
        claim_leaf=sdk.Leaf(
            output="82012088a914b566a3eecce809896361988823cd2f423fe800e788201b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078fac",
            version=192,
        ),
        refund_leaf=sdk.Leaf(
            output="204d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766ad02f401b1",
            version=192,
        ),
    ),
    lockup_address="bcrt1p4z7tpent68fd7qy2d3gyhgn2nzjeexpdekfukdd3j8pd0ffumryslwt9nk",
    refund_public_key="024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766",
    timeout_block_height=500,
    onchain_amount=1000000,
    blinding_key=None,
    asset_id=None,
    fee_asset_id=None,
    swap_auth=None,
    rgb=LOCK,
)


def expect_error(description, build, fragment):
    try:
        build()
    except Exception as error:
        assert fragment in str(error), f"{description}: {error}"
        return
    raise AssertionError(f"{description} should have been rejected")


def color(psbt_base64):
    """What rgb-lib's psbt_op_prepare does: write a commitment into output 0."""
    data = base64.b64decode(psbt_base64)
    assert data[:7].hex() == "70736274ff0100"
    tx_len, width = data[7], 1
    assert tx_len < 0xFD
    start = 7 + width
    tx = data[start : start + tx_len]
    empty = bytes.fromhex("0000000000000000026a00")
    at = tx.index(empty)
    committed = bytes(8) + bytes([0x22, 0x6A, 0x20]) + bytes([0xAB]) * 32
    colored = tx[:at] + committed + tx[at + len(empty) :]
    assert len(colored) < 0xFD
    return base64.b64encode(
        data[:7] + bytes([len(colored)]) + colored + data[start + tx_len :]
    ).decode()


chain = sdk.BitcoinChain.BITCOIN_REGTEST
keys = sdk.KeyPair.from_secret_key(TAKER_SECRET)
preimage = sdk.Preimage.from_bytes(PREIMAGE)

sdk.rgb_check_recipient_script(LOCK, LOCK.script_pubkey)
expect_error(
    "a recipient id for another script",
    lambda: sdk.rgb_check_recipient_script(LOCK, DEST),
    "recipientId",
)

spend = sdk.RgbHtlcSpend.claim(chain, RESPONSE, TAKER_PUBKEY, LOCK_TX_HEX, DEST, None)
assert spend.fee_sat() + 546 == LOCK.htlc_sat, spend.fee_sat()
psbt = spend.psbt()

expect_error(
    "an uncolored claim",
    lambda: spend.sign_colored_tx(psbt, keys, preimage),
    "burn the asset",
)
expect_error(
    "a claim without the preimage",
    lambda: spend.sign_colored_tx(color(psbt), keys, None),
    "preimage",
)

tx = spend.sign_colored_tx(color(psbt), keys, preimage)
assert PREIMAGE.hex() in tx.hex(), "the claim witness reveals the preimage"
assert ("6a20" + "ab" * 32) in tx.hex(), "the commitment is kept"
assert len(tx.txid()) == 64

assert spend.sign_colored(color(psbt), keys, preimage) != color(psbt)

expect_error(
    "a transaction that does not pay the HTLC",
    lambda: sdk.RgbHtlcSpend.claim(
        chain,
        RESPONSE,
        TAKER_PUBKEY,
        LOCK_TX_HEX.replace(LOCK.script_pubkey, DEST),
        DEST,
        None,
    ),
    "no output paying the HTLC",
)

# Defaults stay the SDK's: no contract pinned, a 10 000 sat submarine cap.
expectations = sdk.RgbLockExpectations()
assert expectations.asset_id is None and expectations.max_submarine_htlc_sat is None
