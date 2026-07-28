#!/usr/bin/env python3
"""Static invariants for the tag-only release workflow."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yaml"

REQUIRED_SNIPPETS = (
    'tags:\n      - "v*"',
    "group: release-${{ github.ref }}",
    "cancel-in-progress: false",
    'PYPI_PUBLISH_ENABLED: "false"',
    'NPM_PUBLISH_ENABLED: "false"',
    "release-ready:",
    "registry-publish-complete:",
    "stage-github-release:",
    "release-build-python-wheel-${{ matrix.name }}-${{ github.run_attempt }}",
    "release-bundle-${{ github.ref_name }}-${{ github.run_attempt }}",
    "scripts/assemble_release.py",
    "SHA256SUMS",
    "release.spdx.json",
)


def validate(contents: str) -> None:
    missing = [snippet for snippet in REQUIRED_SNIPPETS if snippet not in contents]
    if missing:
        raise ValueError(f"release workflow is missing invariants: {missing}")
    if "--skip-existing" in contents:
        raise ValueError("release workflow must never skip an existing version")
    mutable_actions = re.findall(r"uses:\s+[^@\s]+@([^\s#]+)", contents)
    invalid = [ref for ref in mutable_actions if not re.fullmatch(r"[0-9a-f]{40}", ref)]
    if invalid:
        raise ValueError(f"release workflow has mutable action refs: {invalid}")
    release_job = contents.split("  stage-github-release:", 1)[1]
    if "needs: registry-publish-complete" not in release_job:
        raise ValueError("GitHub release must depend on the registry completion gate")
    registry_job = contents.split("  registry-publish-complete:", 1)[1].split(
        "  stage-github-release:", 1
    )[0]
    if "needs: release-ready" not in registry_job:
        raise ValueError("registry completion gate must depend on release-ready")


def main() -> int:
    try:
        validate(WORKFLOW.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("Validated release workflow invariants")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
