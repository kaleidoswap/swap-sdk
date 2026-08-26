# Partner attribution: the organization API key, from the UniFFI surface.
#
# Deliberately does not `import common`: nothing here needs the regtest
# environment. Every check runs on the argument itself, before any request.

import inspect

import kaleidorg_swap_sdk

KEY = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz"
MAKER_URL = "https://maker.signet.kaleidoswap.com/v2"


def expect_error(description, build):
    try:
        build()
    except Exception:
        return
    raise AssertionError(f"{description} should have been rejected")


client = kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(MAKER_URL, KEY, None)

# The public half of the key is readable, so a caller can assert at start-up
# that they configured the environment they meant to.
assert client.api_key_environment() == "test", client.api_key_environment()
assert client.api_key_id() == "01KZZYB138E7C3HZX7Q1YBGAQG", client.api_key_id()

# ...and the secret half is not reachable from Python at all: no exported method
# returns it. Checked by calling everything the object will answer with no
# arguments, rather than by eyeballing `str(client)` — UniFFI emits `__str__`
# only for an object that exports `Display`, and this one does not, so a check on
# the rendered form would pass just as well against a client that handed the key
# straight back.
readable = [str(client), repr(client)]
for name in dir(client):
    if name.startswith("_"):
        continue
    attribute = getattr(client, name)
    try:
        value = attribute() if callable(attribute) else attribute
    except TypeError:
        # Takes arguments, so it is not an accessor.
        continue
    if inspect.iscoroutine(value):
        # A swap route, not an accessor. Closed rather than left to warn.
        value.close()
        continue
    readable.append(str(value))

assert not any("s3cr3t" in value for value in readable), readable
# ...and the scan is worth something: it does reach the half that is public.
assert any("01KZZYB138E7C3HZX7Q1YBGAQG" in value for value in readable), readable

# The generic client authenticates nothing: that is what keeps it usable against
# a Boltz maker, which has no notion of an organization key.
generic = kaleidorg_swap_sdk.BoltzApiClientV2(MAKER_URL, None)
assert generic.api_key_environment() is None
assert generic.api_key_id() is None

# A value that cannot be a key fails here rather than at the maker, which
# answers `401` for a revoked key too — so a typo would read as a suspended
# organization.
for bad_key in [
    "",
    "sk_test_abc_secret",
    "kld_staging_abc_secret",
    "kld_test_abc",
    "kld_test__secret",
]:
    expect_error(
        f"api key {bad_key!r}",
        lambda bad_key=bad_key: kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
            MAKER_URL, bad_key, None
        ),
    )

# A bearer credential over plain HTTP is readable by anything on the path, and
# the key is permanent until revoked.
expect_error(
    "a plain-HTTP maker",
    lambda: kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
        "http://maker.signet.kaleidoswap.com/v2", KEY, None
    ),
)

# Userinfo in the URL becomes an `Authorization: Basic ...` of its own, which
# would displace the key and leave the swap unattributed.
expect_error(
    "a maker URL carrying userinfo",
    lambda: kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
        "https://ci:hunter2@maker.signet.kaleidoswap.com/v2", KEY, None
    ),
)

# Loopback is the regtest harness, where the "network" is a socket on this
# machine.
local = kaleidorg_swap_sdk.BoltzApiClientV2.kaleido_maker(
    "http://127.0.0.1:9001/v2", KEY, None
)
assert local.api_key_environment() == "test"

print("kaleido_auth: ok")
