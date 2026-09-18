#!/usr/bin/env python3
"""The React Native package's release inventory, and the binding between its
npm tarball and the native archives attached beside it.

The tarball is small — TypeScript, generated C++, a podspec — because it ships
no compiled code. Its `postinstall` fetches the `.so` and `.a` archives from the
GitHub release and accepts each only at the SHA-256 recorded in a manifest that
travelled inside the tarball. That makes the release exactly as good as the
binding between the two: a tarball whose manifest names digests the attached
archives do not have installs nowhere, and finds out on every partner's machine
at once, after publication. So the binding is established here, before the
bundle is sealed, and re-checked from the sealed bytes immediately before the
assets are attached.
"""

from __future__ import annotations

import hashlib
import json
import re
import tarfile
import zipfile
from pathlib import Path

from release_metadata import (
    NATIVE_ARCHIVES,
    NATIVE_MANIFEST,
    react_native_npm_package,
    react_native_npm_tarball_name,
)

# What `npm pack` must produce for the React Native package. The entry points
# are the ones package.json's `exports` map names, the native-module files are
# the ones an app's build system reads, and the two scripts are the whole of
# the install-time code path.
NPM_REQUIRED = {
    "package/LICENSE",
    "package/README.md",
    "package/package.json",
    f"package/{NATIVE_MANIFEST}",
    "package/react-native.config.js",
    "package/KaleidoswapSwapSdk.podspec",
    "package/android/CMakeLists.txt",
    "package/cpp/generated/kaleidorg_swap_sdk.cpp",
    "package/src/index.tsx",
    "package/src/arkade.ts",
    "package/lib/module/index.js",
    "package/lib/module/arkade.js",
    "package/lib/commonjs/index.js",
    "package/lib/commonjs/arkade.js",
    "package/lib/typescript/module/index.d.ts",
    "package/lib/typescript/commonjs/index.d.ts",
    "package/scripts/postinstall.mjs",
    "package/scripts/native-artifacts.mjs",
}
NPM_ALLOWED_PREFIXES = (
    "package/android/",
    "package/cpp/",
    "package/ios/",
    "package/lib/",
    "package/src/",
)
# Compiled code never rides in the tarball — that is the whole reason the
# archives exist — and neither do the archives themselves.
NPM_FORBIDDEN = re.compile(r"/jniLibs/|^package/build/|\.xcframework(/|$)|\.(so|a|zip)$")
POSTINSTALL = "node scripts/postinstall.mjs"

# Each archive's complete file inventory, from ubrn.config.yaml's target lists.
# An archive with a slice missing would install cleanly and fail on the first
# device of that architecture, so it must not be sealed.
ANDROID_ABIS = ("arm64-v8a", "armeabi-v7a", "x86", "x86_64")
ANDROID_LIBRARIES = frozenset(
    f"android/src/main/jniLibs/{abi}/libkaleidorg_swap_sdk.so" for abi in ANDROID_ABIS
)
IOS_FRAMEWORK = "build/KaleidoSwapSdk.xcframework"
IOS_LIBRARIES = frozenset(
    {
        f"{IOS_FRAMEWORK}/Info.plist",
        f"{IOS_FRAMEWORK}/ios-arm64/libkaleidorg_swap_sdk.a",
        f"{IOS_FRAMEWORK}/ios-arm64_x86_64-simulator/libkaleidorg_swap_sdk.a",
    }
)
ARCHIVE_INVENTORY = {
    "android-artifacts.zip": ANDROID_LIBRARIES,
    "ios-artifacts.zip": IOS_LIBRARIES,
}
if set(ARCHIVE_INVENTORY) != set(NATIVE_ARCHIVES):
    raise AssertionError("ARCHIVE_INVENTORY must describe exactly NATIVE_ARCHIVES")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inspect_archive(path: Path, expected: frozenset[str]) -> None:
    """The archive holds exactly the compiled slices, each with bytes in it."""
    require(path.is_file(), f"release bundle has no {path.name}")
    require(zipfile.is_zipfile(path), f"{path.name} is not a zip archive")
    with zipfile.ZipFile(path) as archive:
        members = {
            info.filename: info.file_size
            for info in archive.infolist()
            if not info.is_dir()
        }
    require(
        set(members) == expected,
        f"{path.name} inventory does not match: "
        f"missing {sorted(expected - set(members))}, "
        f"unexpected {sorted(set(members) - expected)}",
    )
    empty = sorted(name for name, size in members.items() if size == 0)
    require(not empty, f"{path.name} has empty members: {empty}")


def inspect_native_archives(directory: Path) -> None:
    for name, expected in ARCHIVE_INVENTORY.items():
        inspect_archive(directory / name, expected)


def load_manifest(data: bytes, version: str) -> dict[str, str]:
    """Parse a native-artifacts manifest; return archive name -> digest."""
    manifest = json.loads(data)
    require(isinstance(manifest, dict), "native artifact manifest must be a JSON object")
    require(manifest.get("schema") == 1, "native artifact manifest schema mismatch")
    require(
        manifest.get("version") == version,
        "native artifact manifest version mismatch",
    )
    artifacts = manifest.get("artifacts")
    require(
        isinstance(artifacts, dict) and set(artifacts) == set(NATIVE_ARCHIVES),
        "native artifact manifest must list exactly the release's native archives",
    )
    digests: dict[str, str] = {}
    for name, entry in artifacts.items():
        digest = entry.get("sha256") if isinstance(entry, dict) else None
        require(
            isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
            f"native artifact manifest has no valid SHA-256 for {name}",
        )
        digests[name] = digest
    return digests


def inspect_tarball(path: Path, version: str) -> bytes:
    """Check the tarball's inventory and identity; return its embedded manifest."""
    with tarfile.open(path, "r:gz") as archive:
        names = set(archive.getnames())
        missing = sorted(NPM_REQUIRED - names)
        require(not missing, f"{path.name} is missing required files: {missing}")
        package_json = archive.extractfile("package/package.json")
        manifest = archive.extractfile(f"package/{NATIVE_MANIFEST}")
        require(
            package_json is not None and manifest is not None,
            f"{path.name} has unreadable package.json or {NATIVE_MANIFEST}",
        )
        metadata = json.load(package_json)
        embedded = manifest.read()
    unexpected = sorted(
        name
        for name in names
        if name not in NPM_REQUIRED
        and not any(name.startswith(prefix) for prefix in NPM_ALLOWED_PREFIXES)
    )
    require(not unexpected, f"{path.name} contains unexpected files: {unexpected}")
    compiled = sorted(name for name in names if NPM_FORBIDDEN.search(name))
    require(
        not compiled,
        f"{path.name} contains compiled code or archives, which must come from "
        f"the release assets instead: {compiled}",
    )
    require(
        metadata.get("name") == react_native_npm_package(),
        "React Native npm package name mismatch",
    )
    require(
        metadata.get("version") == version,
        "React Native npm package version mismatch",
    )
    # Without this script the tarball installs and the app has no native module;
    # nothing else between here and a partner's device would say why.
    require(
        metadata.get("scripts", {}).get("postinstall") == POSTINSTALL,
        f"{path.name} does not run {POSTINSTALL!r} on install",
    )
    return embedded


def verify_native_binding(directory: Path, version: str) -> None:
    """The tarball's manifest names exactly the archives beside it, at the
    digests they actually have."""
    tarball = directory / react_native_npm_tarball_name(version)
    require(
        tarball.is_file(),
        f"release bundle has no React Native npm tarball: {tarball.name}",
    )
    embedded = inspect_tarball(tarball, version)
    digests = load_manifest(embedded, version)
    loose = directory / NATIVE_MANIFEST
    require(loose.is_file(), f"release bundle has no {NATIVE_MANIFEST}")
    # The loose copy is what a human reads on the release page. It must be the
    # same document the tarball will trust, not merely an equivalent one.
    require(
        loose.read_bytes() == embedded,
        f"{NATIVE_MANIFEST} beside the archives differs from the copy inside "
        f"{tarball.name}",
    )
    for name, digest in digests.items():
        path = directory / name
        require(path.is_file(), f"release bundle has no {name}")
        actual = sha256(path)
        require(
            actual == digest,
            f"{name} does not match the digest inside {tarball.name}: "
            f"manifest {digest}, archive {actual}",
        )
