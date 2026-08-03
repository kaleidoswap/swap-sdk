# Releasing the KaleidoSwap SDK

The Rust crate, Python distribution, and npm package use one public version.
The first release line for this repository is `0.1.0`, and stable release tags
use the form `vX.Y.Z`.

The public package names, migrated to the `kaleidorg` identity:

- Rust crate: `kaleidorg-swap-sdk`
- Python distribution: `kaleidorg_swap_sdk` (normalized by PyPI to
  `kaleidorg-swap-sdk`)
- Python import: `kaleidorg_swap_sdk`
- npm package: `@kaleidorg/swap-sdk`

## Version commands

Show every version source:

```sh
make versions
```

Update every public package and lockfile:

```sh
make sync-version VERSION=0.1.0
```

`Cargo.lock`, the Python `uv.lock`, and the npm `package-lock.json` are committed
so release inputs are reproducible.

Validate that all version sources agree:

```sh
make validate-versions
```

Validate a proposed release tag:

```sh
make validate-release-version TAG=v0.1.0
```

Normal pull-request CI runs the version consistency check. The tag release
workflow additionally verifies that the tag commit is reachable from `trunk`,
builds and smoke-tests all artifacts before publishing, and publishes the
already-tested artifacts without rebuilding them.

## Release architecture

A `v*` tag starts `.github/workflows/release.yaml`. That production-only
wrapper calls the read-only `.github/workflows/release-build.yaml` artifact
graph, then owns the protected publisher and GitHub-release jobs. The preflight
rejects a malformed tag, version drift, a commit that is not reachable from
`trunk`, an occupied enabled-registry version, or an invalid publisher flag.

The workflow builds five native Python wheels, one Python sdist, and one npm
tarball. The `release-ready` job downloads those exact files, validates the
complete inventory, repeats clean-install smoke tests, and generates checksums
and release metadata. Registry jobs can only download the resulting validated
bundle; they never check out source or rebuild a package.

The two repository variables below control the available OIDC publishers:

| Variable | Registry | Default |
|---|---|---|
| `NPM_PUBLISH_ENABLED` | npm | `false` |
| `TEST_PYPI_PUBLISH_ENABLED` | TestPyPI | `false` |

An absent variable behaves as `false`. Set a variable to `true` only after its
registry-side trusted publisher has been configured and independently reviewed.
The workflow rejects any value other than the literal `true` or `false`.
Production `PYPI_PUBLISH_ENABLED` is hardcoded to `false` and is not a
repository variable.

Both publisher jobs:

- depend on the common `release-ready` gate;
- use the protected GitHub `release` environment;
- receive job-scoped `id-token: write`, while every other job remains unable to
  request an OIDC token;
- re-verify the sealed bundle's SHA-256 checksums before publishing;
- publish the exact validated archive without `--skip-existing`;
- contain no npm, PyPI, or TestPyPI password or long-lived token.

The npm job pins an OIDC-capable npm CLI and publishes the validated `.tgz`.
npm automatically emits provenance for a public package from this public
repository. The TestPyPI job uses the immutable-pinned PyPA publisher action and
uploads attestations with the exact wheels and sdist.

Production activation requires npm publishing to be enabled; a production tag
cannot create a GitHub-only release. TestPyPI remains optional. After each
enabled publisher succeeds, a separate read-only job downloads the registry
package, matches its bytes and complete inventory against
`release-manifest.json`, and repeats clean-consumer smoke tests. The final
GitHub release is published from the sealed ten-file bundle and finalized
changelog only after every enabled registry and post-publication verifier
succeeds.

## Non-publishing release rehearsal

Both `.github/workflows/release.yaml` and
`.github/workflows/release-rehearsal.yaml` call the same read-only
`.github/workflows/release-build.yaml` artifact graph. The rehearsal passes
`rehearsal: true` and an empty `release_tag`, so preflight derives the intended
tag from the committed manifests and no workflow file carries a version
literal. It does not copy or approximate the release build. The
rehearsal:

- validates the intended tag against all version sources;
- requires the untagged rehearsal commit to be based on `trunk`;
- validates the publisher flags without requiring the version to still be
  unclaimed, so the rehearsal keeps passing after that version ships;
- builds the same five wheels, sdist, npm tarball, and fresh WASM bindings;
- clean-installs the exact Python artifacts and npm tarball;
- initializes the exact npm tarball in both clean Node and headless Firefox
  consumers;
- generates and re-verifies `SHA256SUMS`, `release-manifest.json`, and
  `release.spdx.json`;
- verifies that the intended GitHub release contains exactly seven packages
  and three metadata files.

The reusable build workflow contains no publisher, protected environment,
`id-token: write`, or `contents: write` declaration. Both OIDC publisher jobs,
the registry-completion gate, and GitHub release creation exist only in the
tag-triggered production wrapper. The rehearsal caller has only
`contents: read`; GitHub therefore cannot grant its graph registry or deployment
authority, regardless of repository publisher variables.

A pull request automatically runs the successful rehearsal when release,
binding, or TypeScript packaging inputs change. After this workflow exists on
`trunk`, an operator can run the successful case explicitly:

```sh
gh workflow run release-rehearsal.yaml \
  --repo kaleidoswap/kaleidorg-swap-sdk \
  --ref trunk \
  -f failure_case=none
```

The manual workflow also exposes four deliberate failure cases:

| `failure_case` | Expected stopping point |
|---|---|
| `malformed-tag` | preflight tag-format validation |
| `version-mismatch` | preflight manifest/tag version validation |
| `missing-wheel` | common release-ready inventory gate |
| `npm-smoke` | clean npm consumer test before artifact upload |

For example:

```sh
gh workflow run release-rehearsal.yaml \
  --repo kaleidoswap/kaleidorg-swap-sdk \
  --ref trunk \
  -f failure_case=missing-wheel
```

An unhappy-path run is successful evidence only when the workflow fails at its
documented stopping point, no `release-bundle-*` artifact is produced, no
deployment approval is requested, and no registry or GitHub release changes.
Do not rerun a deliberate failure as though it were an infrastructure failure.

For a successful run, download the `release-bundle-rehearsal-*` artifact and
retain it with the workflow URL. Its manifest binds every filename and SHA-256
to the rehearsed source commit and intended tag. Publication rehearsal on
TestPyPI remains optional because the exact wheels and sdist are already
clean-installed from the workflow bundle; actual TestPyPI publication requires
the pending trusted publisher described below.

## Protected GitHub release boundary

The repository `release` environment is configured with:

- required reviewers `Arshia-r-m`, `bitwalt`, and `darbon`;
- self-review prevention, so the tag pusher cannot approve their own
  deployment;
- a selected-tag deployment policy matching only `v*`.

GitHub enables administrator bypass when an environment is created through its
API, and does not expose that toggle through the environment REST endpoint.
Before enabling either registry variable, a signed-in repository administrator
must open **Settings → Environments → release**, clear **Allow administrators
to bypass configured protection rules**, save, and verify:

```sh
gh api repos/kaleidoswap/kaleidorg-swap-sdk/environments/release \
  --jq '.can_admins_bypass'
```

The result must be `false`. Administrator bypass is currently still enabled, so
this UI-only hardening step is a release-activation blocker.

The repository tag ruleset named `Protect release tags` restricts creation,
updates, and deletion of matching `v*` tags to the three repository
administrators above. These repository settings are external state and must be
audited before every release.

The `trunk` branch must require pull requests, at least one independent
approval, resolved conversations, current required checks, and protection
against force pushes and deletion. Administrators are subject to the same
rules. Do not push a release tag until the protection API confirms those
settings.

## Registry trusted-publisher setup

Trusted-publisher configuration is registry-side state and cannot be committed
to this repository. The identity fields are case-sensitive and must match
exactly.

### TestPyPI

`kaleidorg-swap-sdk` is currently absent from TestPyPI, so a project owner can add
a pending GitHub Actions publisher without uploading a bootstrap package:

| Field | Value |
|---|---|
| PyPI project name | `kaleidorg-swap-sdk` |
| GitHub owner | `kaleidoswap` |
| Repository | `kaleidorg-swap-sdk` |
| Workflow filename | `release.yaml` |
| Environment | `release` |

After a second maintainer verifies the pending publisher, set the repository
variable:

```sh
gh variable set TEST_PYPI_PUBLISH_ENABLED \
  --repo kaleidoswap/kaleidorg-swap-sdk \
  --body true
```

The first successful OIDC upload creates the TestPyPI project and converts the
pending publisher into a normal publisher.

### npm

`@kaleidorg/swap-sdk` is currently absent from npm. npm exposes trusted-publisher
configuration from an existing package's settings and does not document a
pending-publisher flow for creating a package. Therefore the npm variable must
remain false until package ownership has been bootstrapped outside normal
release CI.

The package owner must:

1. Agree on an npm-approved one-time package bootstrap that does not consume the
   intended `0.1.0` release.
2. Keep any interactive bootstrap credential local; never add it to GitHub
   Actions, repository secrets, files, or logs.
3. In the package settings, configure the GitHub Actions trusted publisher:

   | Field | Value |
   |---|---|
   | Organization or user | `kaleidoswap` |
   | Repository | `kaleidorg-swap-sdk` |
   | Workflow filename | `release.yaml` |
   | Environment | `release` |
   | Allowed action | `npm publish` |

4. Change publishing access to require two-factor authentication and disallow
   traditional tokens.
5. Have a second maintainer verify every field, then set
   `NPM_PUBLISH_ENABLED=true` as a repository variable.

This bootstrap is a release blocker, not a reason to add an npm token to the
workflow.

## v0.1.0 activation checklist

The tag is the irreversible release trigger. Run this checklist only after the
release pull request has merged to `trunk`.

1. Confirm the final `0.1.0` changelog, manifests, and lockfiles:

   ```sh
   git switch trunk
   git pull --ff-only origin trunk
   make validate-release-readiness TAG=v0.1.0
   ```

2. Confirm `@kaleidorg/swap-sdk@0.1.0` and
   `kaleidorg_swap_sdk==0.1.0` are still absent from npm and TestPyPI:

   ```sh
   NPM_PUBLISH_ENABLED=false \
   PYPI_PUBLISH_ENABLED=false \
   TEST_PYPI_PUBLISH_ENABLED=false \
   python3 scripts/check_registry_availability.py 0.1.0 --check-test-pypi
   ```

3. Confirm `trunk` branch protection, the active `Protect release tags`
   ruleset, the `v*` release-environment policy, required reviewers, and
   `can_admins_bypass: false`.
4. Have a second maintainer verify the npm trusted-publisher fields. Set
   `NPM_PUBLISH_ENABLED=true`; production activation deliberately fails while
   this variable is false.
5. If the TestPyPI pending publisher is configured and reviewed, set
   `TEST_PYPI_PUBLISH_ENABLED=true`. Otherwise leave it `false`; public PyPI
   remains hardcoded off.
6. Have an administrator create and push the annotated tag:

   ```sh
   git tag -a v0.1.0 -m "KaleidoSwap SDK v0.1.0"
   git push origin v0.1.0
   ```

7. A required reviewer other than the tag pusher approves the `release`
   environment. Do not use administrator bypass.
8. Monitor the tag workflow through artifact construction, OIDC publication,
   registry download/hash verification, clean Node/Firefox and Python consumer
   tests, and final GitHub release publication.
9. Download and independently verify the ten GitHub release assets:

   ```sh
   mkdir release-v0.1.0
   gh release download v0.1.0 \
     --repo kaleidoswap/kaleidorg-swap-sdk \
     --dir release-v0.1.0
   python3 scripts/verify_release_bundle.py release-v0.1.0 \
     --version 0.1.0 \
     --tag v0.1.0 \
     --commit "$(git rev-list -n 1 v0.1.0)"
   ```

10. Retain the workflow URL, registry URLs, `SHA256SUMS`,
    `release-manifest.json`, and `release.spdx.json` in the release record.

## Partial-publication recovery

External registries cannot participate in an atomic transaction. If one
publisher succeeds and another fails:

1. Do not rerun the entire workflow and do not use `--skip-existing`.
2. Preserve the workflow run, validated release bundle, `SHA256SUMS`, and
   `release-manifest.json`.
3. Check each registry and record which exact filenames and hashes were
   accepted.
4. If the failed registry accepted no files, rerun failed jobs only. GitHub
   leaves the successful publisher untouched and retries the failed publisher,
   post-publication verifier, completion gate, and GitHub-release job. This
   works because release artifact names are attempt-independent, so the retried
   jobs download the same sealed bundle the first attempt validated. Never
   rerun *all* jobs: the build jobs would try to re-upload artifact names that
   already exist and fail, which is the intended protection against silently
   rebuilding a published version.
5. If TestPyPI accepted only part of the Python file set, remove that incomplete
   TestPyPI release through its project controls before rerunning failed jobs.
   Never mix files from separate workflow attempts.
6. If npm accepted the tarball, treat that version as immutable. Complete only
   the missing registry from the same validated bundle or prepare a coordinated
   patch release; never rebuild or overwrite the npm version.
7. The GitHub release remains absent because its job depends on the registry
   publication and verification gate. It is created automatically only after
   all enabled registries are consistent.
8. If rerunning failed jobs cannot succeed — for example npm already holds the
   version, so a fresh preflight would correctly reject it — publish the GitHub
   release by hand from the retained bundle. Verify before uploading, and never
   assemble the assets from anything but that bundle:

   ```sh
   gh run download <run-id> \
     --repo kaleidoswap/kaleidorg-swap-sdk \
     --name "release-bundle-v0.1.0" \
     --dir release-v0.1.0
   python3 scripts/verify_release_bundle.py release-v0.1.0 \
     --version 0.1.0 --tag v0.1.0 \
     --commit "$(git rev-list -n 1 v0.1.0)"
   gh release create v0.1.0 release-v0.1.0/* \
     --repo kaleidoswap/kaleidorg-swap-sdk \
     --title "KaleidoSwap SDK v0.1.0" \
     --verify-tag --latest \
     --notes-file <(python3 scripts/release_notes.py 0.1.0)
   ```

## Python registry

The previous distribution name `kaleidoswap_sdk` collided with an existing
public PyPI project: normalized to `kaleidoswap-sdk`, it already holds releases
`0.1.0` through `0.5.6`, whose versions and uploaded filenames cannot be safely
reused. Uploading platform wheels to that old `0.1.0` would have mixed the new
native package with the old universal wheel.

Renaming the distribution to `kaleidorg_swap_sdk` removes that constraint.
`kaleidorg-swap-sdk` is unclaimed on PyPI, so `0.1.0` is publishable under the
new name.

Public PyPI publishing nevertheless remains **disabled by configuration**:
`PYPI_PUBLISH_ENABLED` is `"false"` in the release workflows and asserted in the
registry-completion gate. That is now a deliberate choice pending a trusted
publisher and an explicit decision to publish, not a technical blocker. Until
it is enabled, the Python artifact is validated locally, on TestPyPI, or in a
private registry. Enabling it requires the same bootstrap as npm: create the
PyPI project, configure its trusted publisher for this repository, and flip the
flag.
