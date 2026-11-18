"""Recovery construction through generated native bindings, without create DTOs."""

import copy
import json
from pathlib import Path

import kaleidorg_swap_sdk as sdk

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = json.loads((ROOT / "tests/fixtures/lusdt-v1/wire-contract.json").read_text())
KEYS = FIXTURE["create"]["chain"]["userAmountRequest"]


def details(value, claim):
    tree = value["swapTree"]
    fields = dict(
        tree=sdk.SwapTree(
            claim_leaf=sdk.Leaf(**tree["claimLeaf"]),
            refund_leaf=sdk.Leaf(**tree["refundLeaf"]),
        ),
        amount=value.get(
            "amount", value.get("expectedAmount", value.get("onchainAmount"))
        ),
        key_index=8 if claim else 7,
        transaction=None,
        lockup_address=value.get("lockupAddress", value.get("address")),
        server_public_key=value.get(
            "serverPublicKey",
            value.get("refundPublicKey" if claim else "claimPublicKey"),
        ),
        timeout_block_height=value["timeoutBlockHeight"],
        blinding_key=value.get("blindingKey"),
    )
    if claim:
        return sdk.ClaimDetails(preimage_hash=KEYS["preimageHash"], **fields)
    return sdk.RefundDetails(**fields)


for kind, swap_type in [
    ("submarine", sdk.SwapRestoreType.SUBMARINE),
    ("reverse", sdk.SwapRestoreType.REVERSE),
    ("chain", sdk.SwapRestoreType.CHAIN),
]:
    response = FIXTURE["create"][kind]["response"]
    restored = sdk.SwapRestoreResponse(
        id=response["id"],
        swap_type=swap_type,
        status="swap.created",
        created_at=1,
        _from="BTC" if kind != "submarine" else "L-BTC",
        to="L-BTC" if kind != "submarine" else "BTC",
        invoice=FIXTURE["create"]["submarine"]["request"]["invoice"],
        claim_details=details(response.get("claimDetails", response), True)
        if kind != "submarine"
        else None,
        refund_details=details(response.get("lockupDetails", response), False)
        if kind != "reverse"
        else None,
    )
    sides = (
        [(sdk.Side.LOCKUP, False), (sdk.Side.CLAIM, True)]
        if kind == "chain"
        else [(None, kind == "reverse")]
    )
    for side, claim in sides:
        public_key = KEYS["claimPublicKey" if claim else "refundPublicKey"]
        script = sdk.SwapScript.from_restored(
            sdk.Network.REGTEST, restored, public_key, side
        )
        assert isinstance(script, sdk.SwapScript)
        bad = copy.deepcopy(restored)
        selected = bad.claim_details if claim else bad.refund_details
        selected.timeout_block_height += 1
        try:
            sdk.SwapScript.from_restored(sdk.Network.REGTEST, bad, public_key, side)
        except sdk.Error.Generic as error:
            assert "non-canonical" in str(error), str(error)
        else:
            raise AssertionError("accepted a changed refund timeout")
