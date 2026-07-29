#!/usr/bin/env python3
"""Static invariants for production release and read-only rehearsal workflows."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yaml"
BUILD_WORKFLOW = ROOT / ".github/workflows/release-build.yaml"
REHEARSAL_WORKFLOW = ROOT / ".github/workflows/release-rehearsal.yaml"

PRODUCTION_REQUIRED = (
    'tags:\n      - "v*"',
    "group: release-${{ github.ref }}",
    "cancel-in-progress: false",
    'PYPI_PUBLISH_ENABLED: "false"',
    "NPM_PUBLISH_ENABLED: ${{ vars.NPM_PUBLISH_ENABLED || 'false' }}",
    "TEST_PYPI_PUBLISH_ENABLED: ${{ vars.TEST_PYPI_PUBLISH_ENABLED || 'false' }}",
    "release-activation:",
    "uses: ./.github/workflows/release-build.yaml",
    "publish-npm:",
    "publish-testpypi:",
    "verify-npm:",
    "verify-testpypi:",
    "registry-publish-complete:",
    "publish-github-release:",
    "release-bundle-${{ needs.release-ready.outputs.release_id }}",
    "npm publish release-artifacts/*.tgz --access public",
    "pypa/gh-action-pypi-publish@ba38be9e461d3875417946c167d0b5f3d385a247",
    "scripts/download_published_artifacts.py",
    "scripts/release_notes.py",
    "--latest",
)

BUILD_REQUIRED = (
    "workflow_call:",
    "commit: ${{ steps.release.outputs.commit }}",
    "release-ready:",
    "rehearsal-complete:",
    "release-build-python-wheel-${{ matrix.name }}",
    "release-bundle-${{ env.RELEASE_ID }}",
    "scripts/assemble_release.py",
    "scripts/verify_release_bundle.py",
    "SHA256SUMS",
    "release.spdx.json",
)

REHEARSAL_FAILURE_CASES = {
    "malformed-tag",
    "missing-wheel",
    "none",
    "npm-smoke",
    "version-mismatch",
}


def job_section(contents: str, name: str, next_name: str) -> str:
    start = f"  {name}:"
    end = f"  {next_name}:"
    if start not in contents or end not in contents:
        raise ValueError(f"could not find workflow job boundary {name!r}")
    return contents.split(start, 1)[1].split(end, 1)[0]


def require_snippets(contents: str, snippets: tuple[str, ...], label: str) -> None:
    missing = [snippet for snippet in snippets if snippet not in contents]
    if missing:
        raise ValueError(f"{label} workflow is missing invariants: {missing}")


def validate(
    contents: str,
    rehearsal_contents: str | None = None,
    build_contents: str | None = None,
) -> None:
    if rehearsal_contents is None:
        rehearsal_contents = REHEARSAL_WORKFLOW.read_text(encoding="utf-8")
    if build_contents is None:
        build_contents = BUILD_WORKFLOW.read_text(encoding="utf-8")

    require_snippets(contents, PRODUCTION_REQUIRED, "production release")
    require_snippets(build_contents, BUILD_REQUIRED, "release build")
    combined = contents + build_contents + rehearsal_contents
    if "--skip-existing" in combined:
        raise ValueError("release workflows must never skip an existing version")

    mutable_actions = re.findall(r"uses:\s+[^@\s]+@([^\s#]+)", combined)
    invalid = [ref for ref in mutable_actions if not re.fullmatch(r"[0-9a-f]{40}", ref)]
    if invalid:
        raise ValueError(f"release workflows have mutable action refs: {invalid}")

    if contents.count("id-token: write") != 2:
        raise ValueError("OIDC permission must be scoped to exactly two publish jobs")
    read_only_authority = (
        "id-token: write",
        "environment: release",
        "contents: write",
        "npm publish ",
        "pypa/gh-action-pypi-publish@",
        "gh release create",
    )
    found_read_only_authority = [
        authority
        for authority in read_only_authority
        if authority in build_contents or authority in rehearsal_contents
    ]
    if found_read_only_authority:
        raise ValueError(
            "read-only release build/rehearsal contains release authority: "
            f"{found_read_only_authority}"
        )

    forbidden_credentials = (
        "secrets.NPM",
        "secrets.PYPI",
        "password:",
        "username:",
    )
    found_credentials = [
        credential for credential in forbidden_credentials if credential in combined
    ]
    if found_credentials:
        raise ValueError(
            "release workflows contain stored registry credentials: "
            f"{found_credentials}"
        )

    build_call = job_section(contents, "release-ready", "publish-npm")
    for snippet in (
        "needs: release-activation",
        "permissions:\n      contents: read",
        "uses: ./.github/workflows/release-build.yaml",
        "failure_case: none",
        "rehearsal: false",
        "release_tag: ${{ github.ref_name }}",
    ):
        if snippet not in build_call:
            raise ValueError(f"production build call is missing {snippet!r}")

    npm_job = job_section(contents, "publish-npm", "publish-testpypi")
    test_pypi_job = job_section(contents, "publish-testpypi", "verify-npm")
    for name, job in (("npm", npm_job), ("TestPyPI", test_pypi_job)):
        if "needs: release-ready" not in job:
            raise ValueError(f"{name} publisher must depend on release-ready")
        if "environment: release" not in job:
            raise ValueError(f"{name} publisher must use the release environment")
        if "id-token: write" not in job:
            raise ValueError(f"{name} publisher must have job-scoped OIDC")
        if "sha256sum --check --strict SHA256SUMS" not in job:
            raise ValueError(f"{name} publisher must re-verify the sealed bundle bytes")

    activation_job = job_section(contents, "release-activation", "release-ready")
    for snippet in (
        'test "${NPM_PUBLISH_ENABLED}" = "true"',
        'test "${PYPI_PUBLISH_ENABLED}" = "false"',
        "scripts/release_notes.py",
    ):
        if snippet not in activation_job:
            raise ValueError(f"release activation is missing {snippet!r}")

    npm_verify_job = job_section(contents, "verify-npm", "verify-testpypi")
    test_pypi_verify_job = job_section(
        contents, "verify-testpypi", "registry-publish-complete"
    )
    for name, job, publisher, smoke in (
        ("npm", npm_verify_job, "publish-npm", "smoke-browser-package.mjs"),
        (
            "TestPyPI",
            test_pypi_verify_job,
            "publish-testpypi",
            "smoke_artifact.py",
        ),
    ):
        if "- release-ready" not in job or f"- {publisher}" not in job:
            raise ValueError(
                f"{name} verifier must depend on release-ready and its publisher"
            )
        if "scripts/download_published_artifacts.py" not in job:
            raise ValueError(f"{name} verifier must download registry artifacts")
        if smoke not in job:
            raise ValueError(f"{name} verifier must run a clean-consumer smoke test")

    release_job = contents.split("  publish-github-release:", 1)[1]
    if "- registry-publish-complete" not in release_job:
        raise ValueError("GitHub release must depend on registry completion")
    if "- release-ready" not in release_job:
        raise ValueError("GitHub release must consume build workflow outputs")
    if "--draft" in release_job:
        raise ValueError("production GitHub release must not remain a draft")
    if "--notes-file release-notes.md" not in release_job:
        raise ValueError("GitHub release must use the finalized changelog")
    if "scripts/verify_release_bundle.py" not in release_job:
        raise ValueError("GitHub release must verify the sealed bundle")
    registry_job = job_section(
        contents, "registry-publish-complete", "publish-github-release"
    )
    for dependency in (
        "release-ready",
        "publish-npm",
        "publish-testpypi",
        "verify-npm",
        "verify-testpypi",
    ):
        if f"- {dependency}" not in registry_job:
            raise ValueError(f"registry completion gate must depend on {dependency}")

    typescript_job = job_section(build_contents, "typescript-package", "release-ready")
    if "node scripts/smoke-package.mjs release-dist/*.tgz" not in typescript_job:
        raise ValueError("npm package must pass its clean-consumer smoke test")
    if (
        "node scripts/smoke-browser-package.mjs release-dist/*.tgz"
        not in typescript_job
    ):
        raise ValueError("npm package must pass its clean-browser smoke test")
    if "inputs.rehearsal && inputs.failure_case == 'npm-smoke'" not in typescript_job:
        raise ValueError("rehearsal must expose a deliberate npm smoke failure")

    release_ready_job = job_section(
        build_contents, "release-ready", "rehearsal-complete"
    )
    if (
        "needs:" not in release_ready_job
        or "- typescript-package" not in release_ready_job
    ):
        raise ValueError("release-ready must depend on the npm smoke-tested package")
    if (
        "inputs.rehearsal && inputs.failure_case == 'missing-wheel'"
        not in release_ready_job
    ):
        raise ValueError("rehearsal must expose a deliberate missing-wheel failure")
    if '--commit "${{ needs.preflight.outputs.commit }}"' not in release_ready_job:
        raise ValueError("release manifest must bind to the peeled source commit")

    rehearsal_job = build_contents.split("  rehearsal-complete:", 1)[1]
    for snippet in (
        "if: ${{ inputs.rehearsal }}",
        "- preflight",
        "- release-ready",
        "scripts/verify_release_bundle.py",
        '--commit "${{ needs.preflight.outputs.commit }}"',
    ):
        if snippet not in rehearsal_job:
            raise ValueError(f"rehearsal verification is missing {snippet!r}")

    rehearsal_required = (
        "pull_request:",
        "workflow_dispatch:",
        "uses: ./.github/workflows/release-build.yaml",
        "rehearsal: true",
        "permissions:\n  contents: read",
        "permissions:\n      contents: read",
    )
    require_snippets(rehearsal_contents, rehearsal_required, "release rehearsal")
    failure_cases = set(
        re.findall(
            r"^\s{10}- ([a-z-]+)$",
            rehearsal_contents,
            flags=re.MULTILINE,
        )
    )
    if failure_cases != REHEARSAL_FAILURE_CASES:
        raise ValueError(
            f"release rehearsal failure cases differ: {sorted(failure_cases)}"
        )

    if re.search(r"^\s+release_tag:\s+v[0-9]", rehearsal_contents, flags=re.MULTILINE):
        raise ValueError(
            "release rehearsal must not hardcode a version; leave release_tag "
            "empty so preflight derives it from the committed manifests"
        )
    if "scripts/release_version.py current" not in build_contents:
        raise ValueError("rehearsal preflight must derive its tag from manifests")
    if "--flags-only" not in build_contents:
        raise ValueError(
            "rehearsal preflight must validate publisher flags without requiring "
            "the current version to still be unclaimed"
        )


def main() -> int:
    try:
        validate(
            WORKFLOW.read_text(encoding="utf-8"),
            REHEARSAL_WORKFLOW.read_text(encoding="utf-8"),
            BUILD_WORKFLOW.read_text(encoding="utf-8"),
        )
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("Validated production release and read-only rehearsal workflow invariants")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
