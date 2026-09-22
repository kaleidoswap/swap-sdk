#!/usr/bin/env python3
"""Static invariants for production release and read-only rehearsal workflows."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yaml"
BUILD_WORKFLOW = ROOT / ".github/workflows/release-build.yaml"
REHEARSAL_WORKFLOW = ROOT / ".github/workflows/release-rehearsal.yaml"
# The runbook an operator reads mid-incident. Its publication order is an
# invariant like any other: prose that outlives the job graph it describes is
# worse than no prose, because it is trusted.
RUNBOOK = ROOT / "docs/releasing.md"
# Composite actions the build workflow calls run inside it with its authority,
# so they are held to the read-only build's rules.
ACTIONS_DIR = ROOT / ".github/actions"

# Both npm packages publish under one OIDC grant, and that grant is justified
# only by provenance — so every manifest it publishes must claim it.
NPM_MANIFESTS = (
    "typescript-sdk/package.json",
    "packages/react-native/package.json",
)

# Structural invariants only. Anything that encodes *today's* policy — which
# registries are enabled, which publisher action is used, whether a registry has a
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
    "verify-react-native-install:",
    # Publishing must consume the sealed bundle, never a fresh build.
    "release-bundle-${{ needs.release-ready.outputs.release_id }}",
)

BUILD_REQUIRED = (
    "workflow_call:",
    "commit: ${{ steps.release.outputs.commit }}",
    "release-ready:",
    "rehearsal-complete:",
    "release-build-python-wheel-${{ matrix.name }}",
    "react-native-package:",
    "release-build-react-native-package",
    "release-build-react-native-archives",
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


def job_dependencies(job: str) -> set[str]:
    """Return the direct dependencies from a scalar or list-style needs key."""
    lines = job.splitlines()
    for index, line in enumerate(lines):
        match = re.fullmatch(r"    needs:\s*([a-z][a-z0-9-]*)?", line)
        if match is None:
            continue
        if match.group(1) is not None:
            return {match.group(1)}

        dependencies: set[str] = set()
        for dependency_line in lines[index + 1 :]:
            dependency = re.fullmatch(
                r"      - ([a-z][a-z0-9-]*)", dependency_line
            )
            if dependency is None:
                break
            dependencies.add(dependency.group(1))
        return dependencies
    return set()


def workflow_job_graph(contents: str) -> dict[str, set[str]]:
    """Map each production job to its direct dependencies."""
    # Split at `jobs:` first: `on:` carries two-space keys of its own, and those
    # are not jobs.
    body = "\n" + contents.split("\njobs:\n", 1)[-1].lstrip("\n")
    return {
        name: job_dependencies(job) for name, job in production_jobs(body).items()
    }


def documented_publication_order(runbook: str) -> list[str]:
    """Return the job order the runbook's publication list claims."""
    heading = "## Publication order"
    if heading not in runbook:
        raise ValueError(
            "docs/releasing.md must carry a 'Publication order' section naming "
            "every production job in the order it runs"
        )
    section = runbook.split(heading, 1)[1].split("\n## ", 1)[0]
    return re.findall(r"^\d+\. `([a-z][a-z0-9-]*)`$", section, re.MULTILINE)


def validate_runbook(runbook: str, contents: str) -> None:
    graph = workflow_job_graph(contents)
    documented = documented_publication_order(runbook)
    if set(documented) != set(graph):
        missing = sorted(set(graph) - set(documented))
        unknown = sorted(set(documented) - set(graph))
        raise ValueError(
            "docs/releasing.md publication order must list every production job: "
            f"missing {missing}, unknown {unknown}"
        )
    if len(documented) != len(set(documented)):
        raise ValueError("docs/releasing.md publication order repeats a job")
    published: set[str] = set()
    for name in documented:
        early = sorted(graph[name] - published)
        if early:
            raise ValueError(
                f"docs/releasing.md lists {name!r} before its dependencies "
                f"{early}; the runbook's publication order contradicts the "
                "workflow's needs graph"
            )
        published.add(name)


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


def composite_actions() -> str:
    if not ACTIONS_DIR.is_dir():
        return ""
    return "".join(
        path.read_text(encoding="utf-8")
        for path in sorted(ACTIONS_DIR.glob("*/action.yml"))
    )


def validate(
    contents: str,
    rehearsal_contents: str | None = None,
    build_contents: str | None = None,
    actions_contents: str | None = None,
    runbook_contents: str | None = None,
) -> None:
    if rehearsal_contents is None:
        rehearsal_contents = REHEARSAL_WORKFLOW.read_text(encoding="utf-8")
    if build_contents is None:
        build_contents = BUILD_WORKFLOW.read_text(encoding="utf-8")
    if actions_contents is None:
        actions_contents = composite_actions()
    if runbook_contents is None:
        runbook_contents = RUNBOOK.read_text(encoding="utf-8")

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
        reads_via_script = "check_registry_availability.py" in workflow
        # Anchor to the start of the YAML key so a longer flag name cannot
        # satisfy a shorter one by substring match.
        undeclared = [
            flag_name
            for flag_name in REGISTRY_FLAGS
            # A flag must be declared if this workflow either hands it to
            # check_registry_availability.py through the environment, or reads it
            # directly in shell. Missing either way, the value is empty at run
            # time and only surfaces in CI.
            if (reads_via_script or f"${{{flag_name}}}" in workflow)
            and not re.search(rf"^\s*{flag_name}:", workflow, re.MULTILINE)
        ]
        if undeclared:
            raise ValueError(
                f"{label} workflow reads publisher flags from the environment "
                f"but does not declare them: {undeclared}"
            )
    combined = contents + build_contents + rehearsal_contents + actions_contents
    # Both spellings. The CLI flag is what twine/npm take, but the PyPA action
    # is configured through a YAML input — and `skip-existing: true` is exactly
    # the edit someone reaches for after a half-failed release, which would
    # otherwise sail past this gate.
    for spelling in ("--skip-existing", "skip-existing:"):
        if spelling in combined:
            raise ValueError(
                f"release workflows must never skip an existing version ({spelling})"
            )

    # npm-package-arg only reads an argument as a file when it starts with
    # `./`, `../`, `~/`, `/` or a drive letter. A bare `release-artifacts/x.tgz`
    # matches the GitHub `owner/repo` shorthand, so npm resolves it as a git
    # dependency and exits 128 having uploaded nothing. Nothing else here
    # catches that: the rehearsal never publishes, so only a real tag can
    # discover it — and by then PyPI has already gone out.
    publishes = re.findall(r"npm publish\s+(\S+)", contents)
    for argument in publishes:
        if not re.match(r"\./|\.\./|~/|/|[a-zA-Z]:", argument):
            raise ValueError(
                "npm publish must be given an explicit file path, or npm reads "
                f"it as a git shorthand (found {argument!r}; prefix it with ./)"
            )
    # Two packages, one bundle, one publish job. A glob would hand npm both
    # tarballs in one argument list, and a single line would ship one package
    # while the other's version stays unclaimed and the release page points at
    # archives no tarball can fetch.
    if len(publishes) != 2 or not any("react-native" in path for path in publishes):
        raise ValueError(
            "publish-npm must publish both npm tarballs by explicit path, the "
            f"browser package and the React Native package (found {publishes})"
        )

    mutable_actions = re.findall(r"uses:\s+[^@\s]+@([^\s#]+)", combined)
    invalid = [ref for ref in mutable_actions if not re.fullmatch(r"[0-9a-f]{40}", ref)]
    if invalid:
        raise ValueError(f"release workflows have mutable action refs: {invalid}")

    # Count real permission keys, not substrings: a comment mentioning
    # "id-token: write" must not inflate the total.
    oidc_grants = re.findall(r"^\s*id-token: write\s*$", contents, re.MULTILINE)
    if len(oidc_grants) != 1:
        raise ValueError(
            "OIDC permission must be scoped to exactly the npm publish job "
            f"(found {len(oidc_grants)})"
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
        if authority in build_contents
        or authority in rehearsal_contents
        or authority in actions_contents
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
    ):
        dependencies = job_dependencies(job)
        if "release-ready" not in dependencies:
            raise ValueError(f"{name} publisher must depend on release-ready")
        if "publish-github-release" not in dependencies:
            raise ValueError(
                f"{name} publisher must run after the GitHub release, so native "
                "archives exist before a registry package can be installed"
            )
        if "environment: release" not in job:
            raise ValueError(f"{name} publisher must use the release environment")
        if name == "npm" and "id-token: write" not in job:
            raise ValueError("npm publisher must have job-scoped OIDC for provenance")
        if name == "npm":
            # The npm job's OIDC scope is justified *only* by provenance. If that
            # field is dropped from a package.json the scope becomes exactly the
            # unused privilege the PyPI job's comment condemns, and nothing else
            # would notice.
            for manifest_path in NPM_MANIFESTS:
                manifest = json.loads((ROOT / manifest_path).read_text(encoding="utf-8"))
                if manifest.get("publishConfig", {}).get("provenance") is not True:
                    raise ValueError(
                        f"npm publisher holds id-token: write, but {manifest_path} "
                        "does not set publishConfig.provenance — either restore it "
                        "or drop the unused OIDC scope"
                    )
        if name != "npm" and re.search(r"^\s*id-token: write\s*$", job, re.MULTILINE):
            raise ValueError(f"{name} publisher must not request unused OIDC scope")
        if "sha256sum --check --strict SHA256SUMS" not in job:
            raise ValueError(f"{name} publisher must re-verify the sealed bundle bytes")

    activation_job = jobs["release-activation"]
    for snippet in (
        'test "${NPM_PUBLISH_ENABLED}" = "true"',
        'case "${PYPI_PUBLISH_ENABLED}" in',
        "scripts/release_notes.py",
    ):
        if snippet not in activation_job:
            raise ValueError(f"release activation is missing {snippet!r}")

    for name, job, publisher, smoke in (
        ("npm", jobs["verify-npm"], "publish-npm", "smoke-browser-package.mjs"),
        ("PyPI", jobs["verify-pypi"], "publish-pypi", "smoke_artifact.py"),
    ):
        if "- release-ready" not in job or f"- {publisher}" not in job:
            raise ValueError(
                f"{name} verifier must depend on release-ready and its publisher"
            )
        if "scripts/download_published_artifacts.py" not in job:
            raise ValueError(f"{name} verifier must download registry artifacts")
        if smoke not in job:
            raise ValueError(f"{name} verifier must run a clean-consumer smoke test")
    if "smoke-react-native-package.mjs" not in jobs["verify-npm"]:
        raise ValueError(
            "npm verifier must smoke-test the published React Native tarball"
        )

    # The one job that runs the React Native package's postinstall for real:
    # after the GitHub release exists, a clean consumer installs the published
    # version with lifecycle scripts on, and the archives must arrive. Every
    # earlier check proves bytes; this one proves a partner's `npm install`.
    install_job = jobs["verify-react-native-install"]
    install_dependencies = job_dependencies(install_job)
    if (
        "publish-github-release" not in install_dependencies
        or "registry-publish-complete" not in install_dependencies
        or "release-ready" not in install_dependencies
    ):
        raise ValueError(
            "React Native install verification must run after the GitHub release "
            "and registry publication both complete, and consume the build "
            "workflow outputs"
        )
    if "smoke-react-native-install.mjs" not in install_job:
        raise ValueError(
            "React Native install verification must install the published "
            "package from the registry"
        )
    if "--ignore-scripts" in install_job:
        raise ValueError(
            "React Native install verification must not disable the postinstall "
            "it exists to prove"
        )

    release_job = jobs["publish-github-release"]
    release_dependencies = job_dependencies(release_job)
    if "release-ready" not in release_dependencies:
        raise ValueError("GitHub release must consume build workflow outputs")
    if "registry-publish-complete" in release_dependencies:
        raise ValueError(
            "GitHub release must precede registry publication, so the React "
            "Native package cannot be published before its native archives exist"
        )
    # Anywhere in the file, not just this job: a draft flag has no legitimate
    # home in a production release workflow, and a job-scoped check silently
    # stops watching the moment a job is added after this one.
    if "--draft" in contents:
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
        "verify-npm",
        "verify-pypi",
    ):
        if f"- {dependency}" not in registry_job:
            raise ValueError(f"registry completion gate must depend on {dependency}")

    react_native_job = job_section(
        build_contents, "react-native-package", "typescript-package"
    )
    if "uses: ./.github/actions/react-native-build" not in react_native_job:
        raise ValueError(
            "React Native release build must use the shared composite action, so "
            "it cannot drift from the pull-request build"
        )
    if (
        "node scripts/smoke-react-native-package.mjs release-dist/*.tgz"
        not in react_native_job
    ):
        raise ValueError("React Native package must pass its clean-consumer smoke test")

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
        or "- react-native-package" not in release_ready_job
    ):
        raise ValueError("release-ready must depend on both npm smoke-tested packages")
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

    # Last: a contradictory job graph should report its own failure first, not
    # surface as a documentation complaint.
    validate_runbook(runbook_contents, contents)


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
