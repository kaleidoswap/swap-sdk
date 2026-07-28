# Releasing the KaleidoSwap SDK

The Rust crate, Python distribution, and npm package use one public version.
The first release line for this repository is `0.1.0`, and stable release tags
use the form `vX.Y.Z`.

The public package names remain unchanged:

- Rust crate: `kaleidoswap-sdk`
- Python distribution: `kaleidoswap_sdk` (normalized by PyPI to
  `kaleidoswap-sdk`)
- Python import: `kaleidoswap_sdk`
- npm package: `@kaleidoswap/sdk`

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

A `v*` tag starts `.github/workflows/release.yaml`. The preflight rejects a
malformed tag, version drift, a commit that is not reachable from `trunk`, an
occupied enabled-registry version, or an invalid publisher flag.

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
- publish the exact validated archive without `--skip-existing`;
- contain no npm, PyPI, or TestPyPI password or long-lived token.

The npm job pins an OIDC-capable npm CLI and publishes the validated `.tgz`.
npm automatically emits provenance for a public package from this public
repository. The TestPyPI job uses the immutable-pinned PyPA publisher action and
uploads attestations with the exact wheels and sdist.

The registry-completion gate accepts a skipped publisher only when its
repository variable is false. The draft GitHub release is created only after
every enabled publisher reports success.

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
gh api repos/kaleidoswap/kaleidoswap-sdk/environments/release \
  --jq '.can_admins_bypass'
```

The result must be `false`. Administrator bypass is currently still enabled, so
this UI-only hardening step is a release-activation blocker.

The repository tag ruleset named `Protect release tags` restricts creation,
updates, and deletion of matching `v*` tags to the three repository
administrators above. These repository settings are external state and must be
audited before every release.

## Registry trusted-publisher setup

Trusted-publisher configuration is registry-side state and cannot be committed
to this repository. The identity fields are case-sensitive and must match
exactly.

### TestPyPI

`kaleidoswap-sdk` is currently absent from TestPyPI, so a project owner can add
a pending GitHub Actions publisher without uploading a bootstrap package:

| Field | Value |
|---|---|
| PyPI project name | `kaleidoswap-sdk` |
| GitHub owner | `kaleidoswap` |
| Repository | `kaleidoswap-sdk` |
| Workflow filename | `release.yaml` |
| Environment | `release` |

After a second maintainer verifies the pending publisher, set the repository
variable:

```sh
gh variable set TEST_PYPI_PUBLISH_ENABLED \
  --repo kaleidoswap/kaleidoswap-sdk \
  --body true
```

The first successful OIDC upload creates the TestPyPI project and converts the
pending publisher into a normal publisher.

### npm

`@kaleidoswap/sdk` is currently absent from npm. npm exposes trusted-publisher
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
   | Repository | `kaleidoswap-sdk` |
   | Workflow filename | `release.yaml` |
   | Environment | `release` |
   | Allowed action | `npm publish` |

4. Change publishing access to require two-factor authentication and disallow
   traditional tokens.
5. Have a second maintainer verify every field, then set
   `NPM_PUBLISH_ENABLED=true` as a repository variable.

This bootstrap is a release blocker, not a reason to add an npm token to the
workflow.

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
   completion gate, and GitHub-release stage.
5. If TestPyPI accepted only part of the Python file set, remove that incomplete
   TestPyPI release through its project controls before rerunning failed jobs.
   Never mix files from separate workflow attempts.
6. If npm accepted the tarball, treat that version as immutable. Complete only
   the missing registry from the same validated bundle or prepare a coordinated
   patch release; never rebuild or overwrite the npm version.
7. The GitHub release remains absent because its job depends on the
   registry-completion gate. Create it only after all enabled registries are
   consistent.

## Public PyPI blocker

The normalized PyPI project name `kaleidoswap-sdk` already contains releases
from `0.1.0` through `0.5.6`. PyPI package versions and uploaded filenames
cannot be safely reused. In particular, uploading platform wheels to the old
`0.1.0` release would mix the new native package with the old universal wheel.

Therefore public PyPI publishing must remain disabled while both of these
requirements are in force:

1. Keep the Python distribution name `kaleidoswap_sdk`.
2. Start the new release line at `0.1.0`.

The Python `0.1.0` artifact can be tested locally, on TestPyPI, or in a private
registry. Enabling production PyPI requires either selecting a new distribution
name or continuing the existing name above `0.5.6`. This restriction does not
affect the Rust version, GitHub release, or `@kaleidoswap/sdk@0.1.0` on npm.
