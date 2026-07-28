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
    "NPM_PUBLISH_ENABLED: ${{ vars.NPM_PUBLISH_ENABLED || 'false' }}",
    "TEST_PYPI_PUBLISH_ENABLED: ${{ vars.TEST_PYPI_PUBLISH_ENABLED || 'false' }}",
    "release-ready:",
    "publish-npm:",
    "publish-testpypi:",
    "registry-publish-complete:",
    "stage-github-release:",
    "release-build-python-wheel-${{ matrix.name }}-${{ github.run_attempt }}",
    "release-bundle-${{ github.ref_name }}-${{ github.run_attempt }}",
    "scripts/assemble_release.py",
    "SHA256SUMS",
    "release.spdx.json",
    "npm publish release-artifacts/*.tgz --access public",
    "pypa/gh-action-pypi-publish@ba38be9e461d3875417946c167d0b5f3d385a247",
)


def job_section(contents: str, name: str, next_name: str) -> str:
    start = f"  {name}:"
    end = f"  {next_name}:"
    if start not in contents or end not in contents:
        raise ValueError(f"could not find workflow job boundary {name!r}")
    return contents.split(start, 1)[1].split(end, 1)[0]


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
    if contents.count("id-token: write") != 2:
        raise ValueError("OIDC permission must be scoped to exactly two publish jobs")
    forbidden_credentials = (
        "secrets.NPM",
        "secrets.PYPI",
        "password:",
        "username:",
    )
    found_credentials = [
        credential for credential in forbidden_credentials if credential in contents
    ]
    if found_credentials:
        raise ValueError(
            f"release workflow contains stored registry credentials: "
            f"{found_credentials}"
        )
    npm_job = job_section(contents, "publish-npm", "publish-testpypi")
    test_pypi_job = job_section(
        contents, "publish-testpypi", "registry-publish-complete"
    )
    for name, job in (("npm", npm_job), ("TestPyPI", test_pypi_job)):
        if "needs: release-ready" not in job:
            raise ValueError(f"{name} publisher must depend on release-ready")
        if "environment: release" not in job:
            raise ValueError(f"{name} publisher must use the release environment")
        if "id-token: write" not in job:
            raise ValueError(f"{name} publisher must have job-scoped OIDC")
    release_job = contents.split("  stage-github-release:", 1)[1]
    if "needs: registry-publish-complete" not in release_job:
        raise ValueError("GitHub release must depend on the registry completion gate")
    registry_job = job_section(
        contents, "registry-publish-complete", "stage-github-release"
    )
    if "- release-ready" not in registry_job:
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
