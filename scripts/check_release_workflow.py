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

# Structural invariants only. Anything that encodes *today's* policy — which
# registries are enabled, which publisher action is used, whether TestPyPI has a
# job at all — is deliberately absent: those are asserted at runtime by the
# workflow's own `test` steps and by check_registry_availability.py, so
# restating them here only made cosmetic edits fail the lint gate.
#
# The load-bearing checks are not in this list; they are the forbidden-pattern
# assertions in validate(): no `--skip-existing`, no mutable action refs, no
# stored registry credentials, and `id-token: write` scoped to exactly the jobs
# that publish.
PRODUCTION_REQUIRED = (
    # Only a v* tag may start a release, and runs must not cancel each other.
    "tags:",
    '- "v*"',
    "group: release-${{ github.ref }}",
    "cancel-in-progress: false",
    # The job graph that enforces build -> validate -> gate -> publish ordering.
    "release-activation:",
    "uses: ./.github/workflows/release-build.yaml",
    "publish-npm:",
    "verify-npm:",
    "registry-publish-complete:",
    "publish-github-release:",
    # Publishing must consume the sealed bundle, never a fresh build.
    "release-bundle-${{ needs.release-ready.outputs.release_id }}",
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

# Read from the environment by scripts/check_registry_availability.py. Every
# workflow that invokes it must declare all three.
REGISTRY_FLAGS = (
    "NPM_PUBLISH_ENABLED",
    "PYPI_PUBLISH_ENABLED",
    "TEST_PYPI_PUBLISH_ENABLED",
)

REHEARSAL_FAILURE_CASES = {
    "malformed-tag",
    "missing-wheel",
    "none",
    "npm-smoke",
    "version-mismatch",
}


def production_jobs(contents: str) -> dict[str, str]:
    """Split the production workflow into its top-level job bodies."""
    names = re.findall(r"^  ([a-z][a-z0-9-]*):$", contents, re.MULTILINE)
    jobs: dict[str, str] = {}
    for index, name in enumerate(names):
        start = contents.index(f"\n  {name}:\n")
        end = (
            contents.index(f"\n  {names[index + 1]}:\n")
            if index + 1 < len(names)
            else len(contents)
        )
        jobs[name] = contents[start:end]
    return jobs


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

    # check_registry_availability.py reads these three from the process
    # environment and refuses to run unless each is explicitly true or false.
    # They have no textual reference in the workflows that invoke it, so a
    # "clean up the unused env var" edit removes them without any local gate
    # noticing — the failure only appears once preflight runs in CI.
    for label, workflow in (
        ("production release", contents),
        ("release build", build_contents),
    ):
        if "check_registry_availability.py" not in workflow:
            continue
        # Anchor to the start of the YAML key: a bare substring test would let
        # TEST_PYPI_PUBLISH_ENABLED satisfy PYPI_PUBLISH_ENABLED.
        undeclared = [
            flag_name
            for flag_name in REGISTRY_FLAGS
            if not re.search(rf"^\s*{flag_name}:", workflow, re.MULTILINE)
        ]
        if undeclared:
            raise ValueError(
                f"{label} workflow runs check_registry_availability.py but does "
                f"not declare the flags it reads from the environment: {undeclared}"
            )
    combined = contents + build_contents + rehearsal_contents
    if "--skip-existing" in combined:
        raise ValueError("release workflows must never skip an existing version")

    mutable_actions = re.findall(r"uses:\s+[^@\s]+@([^\s#]+)", combined)
    invalid = [ref for ref in mutable_actions if not re.fullmatch(r"[0-9a-f]{40}", ref)]
    if invalid:
        raise ValueError(f"release workflows have mutable action refs: {invalid}")

    if contents.count("id-token: write") != 3:
        raise ValueError(
            "OIDC permission must be scoped to exactly the three publish jobs"
        )
    read_only_authority = (
        "id-token: write",
        "environment: release",
        "contents: write",
        "npm publish ",
        "pypa/gh-action-pypi-publish@",
        "gh release create",
        "secrets.NPM_TOKEN",
        "secrets.PYPI_TOKEN",
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

    if "username:" in combined:
        raise ValueError(
            "release workflows must authenticate with a token, not basic auth"
        )

    allowed_secrets = {"NPM_TOKEN", "PYPI_TOKEN"}
    referenced_secrets = set(re.findall(r"secrets\.([A-Z0-9_]+)", combined))
    unexpected = sorted(referenced_secrets - allowed_secrets)
    if unexpected:
        raise ValueError(
            f"release workflows reference unexpected secrets: {unexpected}"
        )

    # A stored credential must only be reachable from a job gated by the
    # protected `release` environment, so publishing still cannot happen without
    # the required review.
    for job_name, job in production_jobs(contents).items():
        used = sorted(
            f"secrets.{name}" for name in allowed_secrets if f"secrets.{name}" in job
        )
        if used and "environment: release" not in job:
            raise ValueError(
                f"job {job_name!r} uses {used} without the protected release "
                "environment, so it could publish without review"
            )

    jobs = production_jobs(contents)
    build_call = jobs["release-ready"]
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

    for name, job in (
        ("npm", jobs["publish-npm"]),
        ("PyPI", jobs["publish-pypi"]),
        ("TestPyPI", jobs["publish-testpypi"]),
    ):
        if "needs: release-ready" not in job:
            raise ValueError(f"{name} publisher must depend on release-ready")
        if "environment: release" not in job:
            raise ValueError(f"{name} publisher must use the release environment")
        if "id-token: write" not in job:
            raise ValueError(f"{name} publisher must have job-scoped OIDC")
        if "sha256sum --check --strict SHA256SUMS" not in job:
            raise ValueError(f"{name} publisher must re-verify the sealed bundle bytes")

    activation_job = jobs["release-activation"]
    for snippet in (
        'test "${NPM_PUBLISH_ENABLED}" = "true"',
        '"${PYPI_PUBLISH_ENABLED}" "${TEST_PYPI_PUBLISH_ENABLED}"',
        "scripts/release_notes.py",
    ):
        if snippet not in activation_job:
            raise ValueError(f"release activation is missing {snippet!r}")

    for name, job, publisher, smoke in (
        ("npm", jobs["verify-npm"], "publish-npm", "smoke-browser-package.mjs"),
        ("PyPI", jobs["verify-pypi"], "publish-pypi", "smoke_artifact.py"),
        (
            "TestPyPI",
            jobs["verify-testpypi"],
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

    release_job = jobs["publish-github-release"]
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
    registry_job = jobs["registry-publish-complete"]
    for dependency in (
        "release-ready",
        "publish-npm",
        "publish-pypi",
        "publish-testpypi",
        "verify-npm",
        "verify-pypi",
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
