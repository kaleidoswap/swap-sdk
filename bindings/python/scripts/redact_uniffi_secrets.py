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
ARG = re.compile(r"^(\s+)self\.(%s),$" % "|".join(SECRET_FIELDS), re.MULTILINE)


def main() -> None:
    source = GENERATED.read_text()
    redacted = ARG.sub(r"\1_redact_secret(self.\2),", source)
    if HELPER_MARKER not in redacted:
        anchor = "\nclass _UniffiRustBuffer(ctypes.Structure)"
        if redacted.count(anchor) != 1:
            raise ValueError("expected exactly one anchor for the redaction helper")
        redacted = redacted.replace(anchor, "\n" + HELPER.rstrip("\n") + "\n\n" + anchor, 1)
    GENERATED.write_text(redacted)


main()
