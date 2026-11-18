# Partner attribution: the organization key, from the UniFFI surface.
#
# Deliberately does not `import common`: nothing here needs the regtest
# environment. Every check runs on the argument itself, before any request.

import kaleidorg_swap_sdk

# The x coordinate of the secp256k1 generator: certainly on the curve.
ORG_KEY = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
MAKER_URL = "https://maker.signet.kaleidoswap.com/v2"


def expect_error(description, build):
    try:
        build()
    except Exception:
        return
    raise AssertionError(f"{description} should have been rejected")


client = kaleidorg_swap_sdk.SwapClient.kaleido_org(MAKER_URL, ORG_KEY, None)

# The key is public, so it reads back, in the lowercase every other system
# stores it in.
assert client.org_key() == ORG_KEY, client.org_key()
upper = kaleidorg_swap_sdk.SwapClient.kaleido_org(MAKER_URL, ORG_KEY.upper(), None)
assert upper.org_key() == ORG_KEY, upper.org_key()

# One client never carries both kinds of attribution: the maker answers a
# request with both `400 attribution_conflict`.
assert client.api_key_id() is None
assert client.api_key_environment() is None
api_client = kaleidorg_swap_sdk.SwapClient.kaleido_maker(
    MAKER_URL, "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t", None
)
assert api_client.org_key() is None

# The generic client attributes nothing.
assert kaleidorg_swap_sdk.SwapClient(MAKER_URL, None).org_key() is None

# A value that cannot be a key fails here, not at the maker as
# `400 org_key_malformed`. The last two are BIP-340's off-curve test vectors.
for bad_key in [
    "",
    ORG_KEY[:63],
    ORG_KEY + "0",
    "02" + ORG_KEY,
    ORG_KEY.replace("7", "g"),
    "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t",
    "eefdea4cdb677750a420fee807eacf21eb9898ae79b9768766e4faa04a2d4a34",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc30",
]:
    expect_error(
        f"org key {bad_key!r}",
        lambda bad_key=bad_key: kaleidorg_swap_sdk.SwapClient.kaleido_org(
            MAKER_URL, bad_key, None
        ),
    )

# Over plain HTTP anything on the path could strip or swap the key.
expect_error(
    "a plain-HTTP maker",
    lambda: kaleidorg_swap_sdk.SwapClient.kaleido_org(
        "http://maker.signet.kaleidoswap.com/v2", ORG_KEY, None
    ),
)

# Loopback is the regtest harness.
local = kaleidorg_swap_sdk.SwapClient.kaleido_org(
    "http://127.0.0.1:9001/v2", ORG_KEY, None
)
assert local.org_key() == ORG_KEY

print("kaleido_org: ok")
