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

The two repository variables below control the available publishers:

| Variable | Registry | Default |
|---|---|---|
| `NPM_PUBLISH_ENABLED` | npm | `false` |
| `PYPI_PUBLISH_ENABLED` | PyPI | `false` |

An absent variable behaves as `false`. Set a variable to `true` only after its
API token is present on the `release` environment and independently reviewed.
The workflow rejects any value other than the literal `true` or `false`.

Both are ordinary repository variables that the production workflow reads
directly (`release.yaml`, `vars.NPM_PUBLISH_ENABLED` / `vars.PYPI_PUBLISH_ENABLED`).
Neither is hardcoded, and neither distinguishes production from any other
context — so a `v*` tag publishes to whichever registries are `true` at the
moment it runs. Read the current values before tagging:

```sh
gh api repos/kaleidoswap/kaleidoswap-sdk/actions/variables \
  --jq '.variables[] | "\(.name) = \(.value)"'
```

The only thing standing between a pushed tag and a permanent publish is the
`release` environment's required reviewers, and an org admin can bypass those.
An npm publish cannot be undone. Never use a tag to test the pipeline — use the
non-publishing rehearsal below.

Both publisher jobs:

- depend on the common `release-ready` gate;
- use the protected GitHub `release` environment;
- for npm only, receive job-scoped `id-token: write` — required for provenance,
  and asserted against `publishConfig.provenance`. The PyPI job requests no OIDC
  scope, because PEP 740 attestations need Trusted Publishing and would be
  unused privilege here. Every other job remains unable to request a token;
- re-verify the sealed bundle's SHA-256 checksums before publishing;
- publish the exact validated archive without `--skip-existing`;
- authenticate with an API token drawn from the protected `release`
  environment, never from repository-level secrets, and never with basic auth.

`scripts/check_release_workflow.py` enforces that: only `NPM_TOKEN` and
`PYPI_TOKEN` may be referenced, only from a job declaring
`environment: release`, and never from the read-only build or rehearsal
workflows. A `username:` key is rejected outright.

Before publishing, the npm job runs `npm whoami` to prove the credential
authenticates. An npm publish cannot be cleanly undone, so a bad token must fail
on a read-only call rather than half-way through the release.

The npm job pins the npm CLI, proves its credential with `npm whoami`, then
publishes the validated `.tgz`. npm emits provenance for a public package from
this public repository. The PyPI job uses the immutable-pinned PyPA publisher
action to upload the exact wheels and sdist. It does **not** attach PEP 740
attestations: those require Trusted Publishing, and the action silently ignores
the `attestations` input when a password is set, so it is pinned to `false`.

Production activation requires npm publishing to be enabled; a production tag
cannot create a GitHub-only release. PyPI is independently gated. After each
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
  --repo kaleidoswap/kaleidoswap-sdk \
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
  --repo kaleidoswap/kaleidoswap-sdk \
  --ref trunk \
  -f failure_case=missing-wheel
```

An unhappy-path run is successful evidence only when the workflow fails at its
documented stopping point, no `release-bundle-*` artifact is produced, no
deployment approval is requested, and no registry or GitHub release changes.
Do not rerun a deliberate failure as though it were an infrastructure failure.

For a successful run, download the `release-bundle-rehearsal-*` artifact and
retain it with the workflow URL. Its manifest binds every filename and SHA-256
to the rehearsed source commit and intended tag. The rehearsal never publishes:
the exact wheels and sdist are already clean-installed from the workflow bundle,
and no registry credential is reachable from the rehearsal graph.

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

The `trunk` branch must require pull requests, at least one independent
approval, resolved conversations, current required checks, and protection
against force pushes and deletion. Administrators are subject to the same
rules. Do not push a release tag until the protection API confirms those
settings.

## Registry credential setup

Publishing authenticates with API tokens held as **`release` environment
secrets**. This is a deliberate tradeoff over OIDC trusted publishing: it avoids
depending on registry-side publisher bootstrap, at the cost of storing long-lived
credentials. See the changelog entry "registry publishing uses stored API tokens"
for the invariants that replace the no-credential property.

Both secrets live on the `release` environment, never at repository level, so
they are unreachable except from a job that environment gates.
`scripts/check_release_workflow.py` enforces that.

### npm — `NPM_TOKEN`

`@kaleidorg/swap-sdk` does not exist on npm yet. npm configures trusted
publishing from an existing package's settings, so a brand-new package cannot be
created by OIDC alone — which is the practical reason this pipeline uses a token.

1. An owner of the `@kaleidorg` scope creates an **automation** token with
   publish rights (granular access tokens scoped to this package are preferable
   to a legacy classic token).
2. Store it as `NPM_TOKEN` on the `release` environment. Never at repository
   level, never in a file, never in logs.
3. Verify it before relying on it — read-only, no publish:

   ```sh
   NPM_TOKEN=<token> npm whoami --registry https://registry.npmjs.org
   ```

   The `publish-npm` job runs the same check before publishing, so a bad token
   fails there rather than part-way through an irreversible release.
4. Set `NPM_PUBLISH_ENABLED=true`.

npm provenance still applies: `publishConfig.provenance` is `true` in
`typescript-sdk/package.json`, and npm derives provenance from the workflow's
OIDC token independently of how it authenticates. That is the only reason
`publish-npm` holds `id-token: write`, and the workflow checker asserts the
linkage — dropping the manifest field fails the gate rather than silently leaving
an unused privilege.

### PyPI — `PYPI_TOKEN`

1. A PyPI account with upload rights for `kaleidorg-swap-sdk` creates an API
   token. The project does not exist yet, so an account-scoped token is needed
   for the first upload; narrow it to the project afterwards.
2. Store it as `PYPI_TOKEN` on the `release` environment.
3. Set `PYPI_PUBLISH_ENABLED=true`.

PyPI has **no read-only way to validate an upload token**, so unlike npm there is
no pre-flight. A bad `PYPI_TOKEN` fails at upload — after npm has already
published. See "Partial-publication recovery" below; note that PyPI never frees a
version number, so a spent `0.1.0` cannot be reused.

PEP 740 attestations are **not** produced. They require Trusted Publishing, and
the PyPA action silently ignores `attestations: true` when a password is set, so
that input is pinned to `false` and the PyPI job requests no OIDC scope.

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
   `kaleidorg_swap_sdk==0.1.0` are still absent from npm and PyPI:

   ```sh
   NPM_PUBLISH_ENABLED=false \
   PYPI_PUBLISH_ENABLED=false \
   python3 scripts/check_registry_availability.py 0.1.0 --check-pypi
   ```

3. Confirm `trunk` branch protection, the active `Protect release tags`
   ruleset, the `v*` release-environment policy, required reviewers, and
   `can_admins_bypass: false`.
4. Have a second maintainer confirm `NPM_TOKEN` is present on the `release`
   environment and scoped to publish `@kaleidorg/swap-sdk`. Set
   `NPM_PUBLISH_ENABLED=true`; production activation deliberately fails while
   this variable is false.
5. To publish Python, confirm `PYPI_TOKEN` is present on the `release`
   environment and set `PYPI_PUBLISH_ENABLED=true`. Otherwise leave it `false`
   and the Python artifacts ship only as GitHub release assets.
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
     --repo kaleidoswap/kaleidoswap-sdk \
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
5. If PyPI accepted only part of the Python file set, that version is spent:
   PyPI does not allow re-uploading a filename, and yanking does not free it.
   Prepare a coordinated patch version rather than attempting to complete the
   partial upload, and never mix files from separate workflow attempts.
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
     --repo kaleidoswap/kaleidoswap-sdk \
     --name "release-bundle-v0.1.0" \
     --dir release-v0.1.0
   python3 scripts/verify_release_bundle.py release-v0.1.0 \
     --version 0.1.0 --tag v0.1.0 \
     --commit "$(git rev-list -n 1 v0.1.0)"
   gh release create v0.1.0 release-v0.1.0/* \
     --repo kaleidoswap/kaleidoswap-sdk \
     --title "KaleidoSwap SDK v0.1.0" \
     --verify-tag --latest \
     --notes-file <(python3 scripts/release_notes.py 0.1.0)
   ```

## Rust distribution

The Rust surface is **git-only**. Nothing in this pipeline runs `cargo package`
or `cargo publish`, and `kaleidorg-swap-sdk` is unclaimed on crates.io.
Consumers take it by tag:

```toml
kaleidorg-swap-sdk = { git = "https://github.com/kaleidoswap/kaleidoswap-sdk", tag = "v0.1.0" }
```

The `repository`, `homepage`, and `include` metadata on the manifests exists so
the crates are publishable *later* without another metadata pass, not because
publishing happens today.

Two things must be resolved before crates.io is possible, so treat this as a
deliberate deferral rather than an oversight:

1. `kaleidorg-swap-sdk-macros` is a path dependency and would have to be
   published first.
2. The "one synchronized version" contract covers three of the five crates. The
   root crate, Python distribution, and npm package share `0.1.0`;
   `bindings`/`bindings-wasm` are independently `0.1.0` and `macros` is `1.0.0`.
   `validate-versions` deliberately checks only the three public surfaces.

## Python registry

The previous distribution name `kaleidoswap_sdk` collided with an existing
public PyPI project: normalized to `kaleidoswap-sdk`, it already holds releases
`0.1.0` through `0.5.6`, whose versions and uploaded filenames cannot be safely
reused. Uploading platform wheels to that old `0.1.0` would have mixed the new
native package with the old universal wheel.

Renaming the distribution to `kaleidorg_swap_sdk` removes that constraint.
`kaleidorg-swap-sdk` is unclaimed on PyPI, so `0.1.0` is publishable under the
new name.

Public PyPI publishing defaults to **off**: `PYPI_PUBLISH_ENABLED` is absent or
`false`, and the registry-completion gate requires the publish and verify jobs to
be `skipped` in that case. Enabling it needs only the `PYPI_TOKEN` secret on the
`release` environment and the variable set to `true`. Until then the Python
artifacts are validated in CI and shipped as GitHub release assets.

A PyPI upload is effectively permanent: a filename cannot be re-uploaded and a
yank does not free the version. Treat enabling this flag as a one-way door for
the version being released.
