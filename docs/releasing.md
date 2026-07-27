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

Normal pull-request CI runs the version consistency check. A future tag release
workflow must additionally verify that the tag commit is reachable from
`trunk`, build and smoke-test all artifacts before publishing, and publish the
already-tested artifacts without rebuilding them.

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
