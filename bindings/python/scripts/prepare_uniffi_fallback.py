#!/usr/bin/env python3
"""Adjust generated UniFFI glue to load Maturin's nested native library."""

from pathlib import Path

FALLBACK = Path("kaleidorg_swap_sdk/_generated_uniffi.py")
WINDOWS_LOADER = """\
        libname = os.path.join(
            os.path.dirname(__file__),
            "{}.dll",
        )
"""
NESTED_WINDOWS_LOADER = '        libname = "{}.dll"\n'
LOCAL_LIBRARY = "    path = os.path.join(os.path.dirname(__file__), libname)\n"
NESTED_LIBRARY = '    path = os.path.join(os.path.dirname(__file__), "kaleidorg_swap_sdk", libname)\n'


def replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise ValueError(f"expected exactly one generated loader fragment: {old!r}")
    return source.replace(old, new)


source = FALLBACK.read_text()
source = replace_once(source, WINDOWS_LOADER, NESTED_WINDOWS_LOADER)
source = replace_once(source, LOCAL_LIBRARY, NESTED_LIBRARY)
FALLBACK.write_text(source)
