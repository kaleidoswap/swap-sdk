"""Saved transaction recovery through the generated UniFFI/Python bindings."""

import json
from pathlib import Path

import kaleidorg_swap_sdk as sdk

fixtures = json.loads(
    (
        Path(__file__).resolve().parents[3]
        / "tests/fixtures/transactions-v1/saved-transactions.json"
    ).read_text()
)
for name, chain in [
    ("bitcoin", sdk.TransactionChain.BITCOIN),
    ("bitcoinLegacy", sdk.TransactionChain.BITCOIN),
    ("liquid", sdk.TransactionChain.LIQUID),
]:
    fixture = fixtures[name]
    tx = sdk.BtcLikeTransaction.from_hex(fixture["hex"], chain)
    assert tx.hex() == fixture["hex"]
    assert tx.txid() == fixture["txid"]
    uppercase = sdk.BtcLikeTransaction.from_hex(fixture["hex"].upper(), chain)
    assert uppercase.hex() == fixture["hex"]
    try:
        sdk.BtcLikeTransaction.from_hex(fixture["hex"] + "00", chain)
    except sdk.Error:
        pass
    else:
        raise AssertionError("trailing bytes must be rejected")

for name, wrong_chain in [
    ("bitcoin", sdk.TransactionChain.LIQUID),
    ("liquid", sdk.TransactionChain.BITCOIN),
]:
    try:
        sdk.BtcLikeTransaction.from_hex(fixtures[name]["hex"], wrong_chain)
    except sdk.Error:
        pass
    else:
        raise AssertionError("wrong transaction family must fail")
