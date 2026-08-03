#!/usr/bin/env python3
"""Shared release identity and platform-tag matchers.

The pre-publication assembler and the post-publication verifier must agree on
three things: the npm package name, the tarball filename npm derives from it,
and which wheel filename counts as which platform. Keeping separate copies let a
package rename break one side silently, and let a compound platform tag pass the
assembler's regex while failing the verifier's `endswith` — a divergence whose
failure lands *after* the artifact is already published.

Read identity from the manifests, the way release_version.py reads versions.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PYTHON_DISTRIBUTION = "kaleidorg_swap_sdk"

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

# The wheel the post-publication verifier installs. Taken from the table above
# rather than restated, so it cannot drift from the assembler's notion of it.
LINUX_X86_64_WHEEL = PLATFORM_MARKERS_BY_LABEL["linux x86_64"]


def npm_package(root: Path | None = None) -> str:
    """The npm package name, read from its manifest instead of duplicated."""
    path = (root or ROOT) / "typescript-sdk/package.json"
    with path.open(encoding="utf-8") as file:
        name = json.load(file).get("name")
    if not isinstance(name, str) or not name:
        raise ValueError(f"{path} declares no package name")
    return name


def npm_tarball_name(version: str, package: str | None = None) -> str:
    """Reproduce `npm pack`'s filename for a (possibly scoped) package.

    npm drops the leading `@` and replaces the scope separator with a dash:
    `@kaleidorg/swap-sdk` at 0.1.0 packs as `kaleidorg-swap-sdk-0.1.0.tgz`.
    """
    name = npm_package() if package is None else package
    return f"{name.lstrip('@').replace('/', '-')}-{version}.tgz"
