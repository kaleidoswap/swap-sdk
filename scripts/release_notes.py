#!/usr/bin/env python3
"""Extract the finalized release notes for one SDK version."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"


def extract_release_notes(contents: str, version: str) -> str:
    heading = re.compile(
        rf"^## \[{re.escape(version)}\](?: - \d{{4}}-\d{{2}}-\d{{2}})?\s*$",
        flags=re.MULTILINE,
    )
    match = heading.search(contents)
    if match is None:
        raise ValueError(f"CHANGELOG.md has no finalized [{version}] section")

    next_heading = re.search(r"^## \[", contents[match.end() :], flags=re.MULTILINE)
    end = match.end() + next_heading.start() if next_heading else len(contents)
    notes = contents[match.end() : end].strip()
    if not notes:
        raise ValueError(f"CHANGELOG.md [{version}] section is empty")
    return notes


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--changelog", type=Path, default=CHANGELOG)
    args = parser.parse_args()
    try:
        contents = args.changelog.read_text(encoding="utf-8")
        notes = extract_release_notes(contents, args.version)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(notes)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
