#!/usr/bin/env python3
"""Validate that a release tag identifies the expected commit on trunk."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def validate_tag(tag: str) -> None:
    subprocess.run(
        [sys.executable, ROOT / "scripts/release_version.py", "validate-tag", tag],
        cwd=ROOT,
        check=True,
    )


def require_ancestor(ancestor: str, descendant: str, message: str) -> None:
    if (
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", ancestor, descendant],
            cwd=ROOT,
            check=False,
        ).returncode
        != 0
    ):
        raise ValueError(message)


def validate_release_ref(tag: str, expected_sha: str, base_ref: str) -> None:
    validate_tag(tag)
    tag_commit = git("rev-parse", f"{tag}^{{commit}}")
    expected_commit = git("rev-parse", f"{expected_sha}^{{commit}}")
    if tag_commit != expected_commit:
        raise ValueError(
            f"{tag} resolves to {tag_commit}, but the workflow is building "
            f"{expected_commit}"
        )
    base_commit = git("rev-parse", f"{base_ref}^{{commit}}")
    require_ancestor(
        tag_commit,
        base_commit,
        f"{tag_commit} is not reachable from {base_ref}",
    )
    print(f"Validated {tag} at {tag_commit}, reachable from {base_ref}")


def validate_rehearsal_ref(tag: str, expected_sha: str, base_ref: str) -> None:
    validate_tag(tag)
    expected_commit = git("rev-parse", f"{expected_sha}^{{commit}}")
    base_commit = git("rev-parse", f"{base_ref}^{{commit}}")
    require_ancestor(
        base_commit,
        expected_commit,
        f"{expected_commit} is not based on {base_ref}",
    )
    print(f"Validated rehearsal for {tag} at {expected_commit}, based on {base_ref}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--rehearsal",
        action="store_true",
        help="validate an untagged source commit based on the protected branch",
    )
    parser.add_argument("tag")
    parser.add_argument("expected_sha")
    parser.add_argument("base_ref")
    args = parser.parse_args()
    try:
        if args.rehearsal:
            validate_rehearsal_ref(args.tag, args.expected_sha, args.base_ref)
        else:
            validate_release_ref(args.tag, args.expected_sha, args.base_ref)
    except (subprocess.CalledProcessError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
