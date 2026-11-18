#!/usr/bin/env python3
"""Shared release identity and platform-tag matchers.

The pre-publication assembler and the post-publication verifier must agree on
three things: the npm package names, the tarball filenames npm derives from
them, and which wheel filename counts as which platform. Keeping separate copies
let a package rename break one side silently, and let a compound platform tag
pass the assembler's regex while failing the verifier's `endswith` — a
divergence whose failure lands *after* the artifact is already published.

Read identity from the manifests, the way release_version.py reads versions.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PYTHON_DISTRIBUTION = "kaleidorg_swap_sdk"

# The two npm packages a release publishes: the browser/Node package over the
# wasm binding, and the React Native package over the UniFFI crate.
TYPESCRIPT_MANIFEST = "typescript-sdk/package.json"
REACT_NATIVE_MANIFEST = "packages/react-native/package.json"
NPM_PACKAGE_COUNT = 2

# The React Native package ships no compiled code. Its `postinstall` fetches
# these archives from the GitHub release by exactly these names, and accepts
# each only at the digest recorded in this manifest, a copy of which rides
# inside the tarball. react_native_release.py binds the three before the bundle
# is sealed and re-checks the binding before the assets are attached.
NATIVE_ARCHIVES = ("android-artifacts.zip", "ios-artifacts.zip")
NATIVE_MANIFEST = "native-artifacts.json"

# Label -> filename matcher. This is the single definition of the release's
# platform inventory; both the wheel-count check and the Linux-wheel selection
# below derive from it.
PLATFORM_MARKERS = (
    ("linux x86_64", re.compile(r"manylinux[^-]*_x86_64\.whl$")),
    ("linux aarch64", re.compile(r"manylinux[^-]*_aarch64\.whl$")),
    ("macOS x86_64", re.compile(r"macosx[^-]*_x86_64\.whl$")),
    ("macOS arm64", re.compile(r"macosx[^-]*_arm64\.whl$")),
    ("Windows x86_64", re.compile(r"win_amd64\.whl$")),
)

PLATFORM_MARKERS_BY_LABEL = dict(PLATFORM_MARKERS)

# One wheel per platform, the sdist, both npm tarballs, and the React Native
# package's native archives with their manifest. Adding a wheel target to
# PLATFORM_MARKERS, or an archive to NATIVE_ARCHIVES, updates the assembler's
# counts, the sealed bundle's expected inventory, and the rehearsal summary
# together.
WHEEL_COUNT = len(PLATFORM_MARKERS)
PACKAGE_COUNT = WHEEL_COUNT + 1 + NPM_PACKAGE_COUNT + len(NATIVE_ARCHIVES) + 1

METADATA_FILES = frozenset(
    {
        "SHA256SUMS",
        "release-manifest.json",
        "release.spdx.json",
    }
)
RELEASE_ASSET_COUNT = PACKAGE_COUNT + len(METADATA_FILES)

# The wheel the post-publication verifier installs. Taken from the table above
# rather than restated, so it cannot drift from the assembler's notion of it.
LINUX_X86_64_WHEEL = PLATFORM_MARKERS_BY_LABEL["linux x86_64"]


def npm_package(root: Path | None = None, manifest: str = TYPESCRIPT_MANIFEST) -> str:
    """An npm package name, read from its manifest instead of duplicated."""
    path = (root or ROOT) / manifest
    with path.open(encoding="utf-8") as file:
        name = json.load(file).get("name")
    if not isinstance(name, str) or not name:
        raise ValueError(f"{path} declares no package name")
    return name


def react_native_npm_package(root: Path | None = None) -> str:
    return npm_package(root, REACT_NATIVE_MANIFEST)


def npm_tarball_name(version: str, package: str | None = None) -> str:
    """Reproduce `npm pack`'s filename for a (possibly scoped) package.

    npm drops the leading `@` and replaces the scope separator with a dash:
    `@kaleidorg/swap-sdk` at 0.1.0 packs as `kaleidorg-swap-sdk-0.1.0.tgz`.
    """
    name = npm_package() if package is None else package
    return f"{name.lstrip('@').replace('/', '-')}-{version}.tgz"


def react_native_npm_tarball_name(version: str) -> str:
    return npm_tarball_name(version, react_native_npm_package())
