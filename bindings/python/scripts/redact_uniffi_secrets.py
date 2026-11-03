#!/usr/bin/env python3
"""Keep credentials and private keys out of the generated `__str__` methods.

UniFFI formats every record field verbatim. The Rust `Debug` impls redact
`swap_auth` and `blinding_key`, so the Python formatters have to as well.
Fields stay readable as attributes for persistence. Idempotent.
"""

import re
from pathlib import Path

GENERATED = Path("kaleidorg_swap_sdk/_generated_uniffi.py")
SECRET_FIELDS = ("swap_auth", "blinding_key")
HELPER_MARKER = "def _redact_secret("
HELPER = '''\
def _redact_secret(value):
    """Render a secret field as present or absent, never as its value."""
    return "None" if value is None else "<redacted>"


'''
FIELD = "|".join(SECRET_FIELDS)
STR_METHOD = re.compile(r"    def __str__\(self\):\n(?:(?!    def ).*\n?)*")
SECRET = re.compile(r"(?<![\w.])(?<!_redact_secret\()self\.(%s)\b" % FIELD)
UNREDACTED = re.compile(r"(?<!_redact_secret\()self\.(%s)\b" % FIELD)


def redact_str_methods(source: str) -> str:
    """Wrap every secret field read inside a `__str__` body, wherever it sits."""
    return STR_METHOD.sub(lambda m: SECRET.sub(r"_redact_secret(self.\1)", m.group(0)), source)


def main() -> None:
    source = GENERATED.read_text()
    redacted = redact_str_methods(source)
    leaked = [m for block in STR_METHOD.findall(redacted) for m in UNREDACTED.findall(block)]
    if leaked:
        raise ValueError(f"a generated __str__ still reads a secret field: {leaked}")
    if HELPER_MARKER not in redacted:
        anchor = "\nclass _UniffiRustBuffer(ctypes.Structure)"
        if redacted.count(anchor) != 1:
            raise ValueError("expected exactly one anchor for the redaction helper")
        redacted = redacted.replace(anchor, "\n" + HELPER.rstrip("\n") + "\n\n" + anchor, 1)
    GENERATED.write_text(redacted)


main()
