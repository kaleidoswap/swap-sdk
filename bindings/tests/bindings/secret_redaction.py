"""Generated records must not print `swap_auth` or a Liquid `blinding_key`.

`str()` on a create response, a restore detail or `TransactionOptions` is how a
caller traces a swap, and it printed the maker's per-swap credential — the
taker's full capability over the swap — and the key that unblinds the Liquid
outputs. The Rust `Debug` impls redact both; the package applies the same
redaction to the generated glue at import time (`_redact.py`). This script
runs against the raw glue the harness generates, so it loads that module by
path and applies it the way the package `__init__` does.
"""

import importlib.util
import sys
from pathlib import Path

import kaleidorg_swap_sdk as sdk

REDACT = (
    Path(__file__).resolve().parents[2] / "python" / "kaleidorg_swap_sdk" / "_redact.py"
)
spec = importlib.util.spec_from_file_location("_redact", REDACT)
redact = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = redact
spec.loader.exec_module(redact)

AUTH = "c0ffee2ac2b0d2ff1a8f5b7e6d4c3b2a19081726354453627180a9b8c7d6e5f4"
BLINDING = "07e3b7a6c5d4f3a2b1908f7e6d5c4b3a2918f7e6d5c4b3a2918f7e6d5c4b3a29"
SERVER_KEY = "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2"
TREE = sdk.SwapTree(
    claim_leaf=sdk.Leaf(output="00", version=196),
    refund_leaf=sdk.Leaf(output="01", version=196),
)


def details(blinding_key):
    return sdk.ChainSwapDetails(
        swap_tree=TREE,
        lockup_address="el1qqlockup",
        server_public_key=SERVER_KEY,
        timeout_block_height=200,
        amount=100_000,
        blinding_key=blinding_key,
        refund_address=None,
        claim_address=None,
        bip21=None,
        asset_id=None,
        fee_asset_id=None,
    )


def chain(swap_auth, blinding_key):
    return sdk.CreateChainResponse(
        id="01KZZYB138E7C3HZX7Q1YBGAQG",
        claim_details=details(blinding_key),
        lockup_details=details(blinding_key),
        swap_auth=swap_auth,
    )


def reverse(blinding_key):
    return sdk.CreateReverseResponse(
        id="reverse-id",
        invoice=None,
        swap_tree=TREE,
        lockup_address="el1qqlockup",
        refund_public_key=SERVER_KEY,
        timeout_block_height=200,
        onchain_amount=100_000,
        blinding_key=blinding_key,
        asset_id=None,
        fee_asset_id=None,
        swap_auth=None,
        rgb=None,
    )


def claim_details(blinding_key):
    return sdk.ClaimDetails(
        detail_type="utxo",
        tree=TREE,
        amount=100_000,
        key_index=7,
        transaction=None,
        lockup_address="el1qqlockup",
        server_public_key=SERVER_KEY,
        timeout_block_height=144,
        blinding_key=blinding_key,
        preimage_hash="ab" * 32,
    )


# The generated shape, captured before the patch so fidelity can be checked.
unpatched = {
    "options": str(sdk.TransactionOptions()),
    "chain": str(chain(None, None)),
    "reverse": str(reverse(None)),
    "claim": str(claim_details(None)),
}
assert AUTH in str(sdk.TransactionOptions(swap_auth=AUTH)), (
    "the raw glue prints the secret"
)

patched = redact.redact_secret_fields(sdk)
for name in (
    "TransactionOptions",
    "CreateSubmarineResponse",
    "CreateReverseResponse",
    "CreateChainResponse",
    "ChainSwapDetails",
    "ClaimDetails",
    "RefundDetails",
):
    assert name in patched, (name, patched)

# Secrets are withheld...
for rendered in (
    str(sdk.TransactionOptions(swap_auth=AUTH)),
    str(chain(AUTH, BLINDING)),
    str(reverse(BLINDING)),
    str(claim_details(BLINDING)),
):
    assert AUTH not in rendered, rendered
    assert BLINDING not in rendered, rendered
    assert "<redacted>" in rendered, rendered
assert "swap_auth=<redacted>" in str(sdk.TransactionOptions(swap_auth=AUTH))
# ...on both chain legs, through the nested record's own __str__...
assert str(chain(AUTH, BLINDING)).count("blinding_key=<redacted>") == 2
assert "swap_auth=<redacted>" in str(chain(AUTH, BLINDING))
# ...while everything else prints as the generated code printed it.
assert str(sdk.TransactionOptions()) == unpatched["options"], unpatched["options"]
assert str(chain(None, None)) == unpatched["chain"]
assert str(reverse(None)) == unpatched["reverse"]
assert str(claim_details(None)) == unpatched["claim"]
assert "swap_auth=None" in str(chain(None, None))
assert "01KZZYB138E7C3HZX7Q1YBGAQG" in str(chain(AUTH, BLINDING))

# Attribute access is untouched, so the credential can still be persisted.
assert sdk.TransactionOptions(swap_auth=AUTH).swap_auth == AUTH
assert chain(AUTH, BLINDING).claim_details.blinding_key == BLINDING

# The default repr never printed fields; it must not start to.
assert AUTH not in repr(sdk.TransactionOptions(swap_auth=AUTH))
assert AUTH not in "%r" % sdk.TransactionOptions(swap_auth=AUTH)
print("secret_redaction: ok")
