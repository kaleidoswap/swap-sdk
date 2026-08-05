#!/usr/bin/env python3
"""Validate and describe the immutable SDK release artifact bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tarfile
from datetime import datetime, timezone
from pathlib import Path

from release_metadata import (
    PLATFORM_MARKERS,
    PYTHON_DISTRIBUTION,
    WHEEL_COUNT,
    npm_package,
)

NPM_REQUIRED = {
    "package/LICENSE",
    "package/README.md",
    "package/dist/index.d.ts",
    "package/dist/index.js",
    "package/package.json",
    "package/vendor/bindings_wasm.d.ts",
    "package/vendor/bindings_wasm.js",
    "package/vendor/bindings_wasm_bg.wasm",
    "package/vendor/bindings_wasm_bg.wasm.d.ts",
}
NPM_ALLOWED_PREFIXES = ("package/dist/", "package/vendor/")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def commit_created(commit: str) -> str:
    result = subprocess.run(
        ["git", "show", "-s", "--format=%cI", commit],
        check=True,
        capture_output=True,
        text=True,
    )
    created = datetime.fromisoformat(result.stdout.strip())
    return created.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def collect_artifacts(directory: Path, version: str) -> list[Path]:
    artifacts = sorted(path for path in directory.iterdir() if path.is_file())
    wheels = [path for path in artifacts if path.suffix == ".whl"]
    sdists = [path for path in artifacts if path.name.endswith(".tar.gz")]
    npm = [path for path in artifacts if path.suffix == ".tgz"]
    require(
        len(wheels) == WHEEL_COUNT,
        f"expected {WHEEL_COUNT} wheels, found {len(wheels)}",
    )
    require(len(sdists) == 1, f"expected one sdist, found {len(sdists)}")
    require(len(npm) == 1, f"expected one npm tarball, found {len(npm)}")
    expected_prefix = f"{PYTHON_DISTRIBUTION}-{version}"
    require(
        all(path.name.startswith(expected_prefix) for path in wheels + sdists),
        "Python artifact name/version mismatch",
    )
    for label, marker in PLATFORM_MARKERS:
        require(
            sum(bool(marker.search(path.name)) for path in wheels) == 1,
            f"expected exactly one {label} wheel",
        )
    inspect_npm(npm[0], version)
    return wheels + sdists + npm


def inspect_npm(path: Path, version: str) -> None:
    with tarfile.open(path, "r:gz") as archive:
        names = set(archive.getnames())
        package_json = archive.extractfile("package/package.json")
        require(package_json is not None, f"{path.name} has no package.json")
        metadata = json.load(package_json)
    missing = sorted(NPM_REQUIRED - names)
    require(not missing, f"{path.name} is missing required files: {missing}")
    unexpected = sorted(
        name
        for name in names
        if name not in NPM_REQUIRED
        and not any(name.startswith(prefix) for prefix in NPM_ALLOWED_PREFIXES)
    )
    require(not unexpected, f"{path.name} contains unexpected files: {unexpected}")
    require(metadata.get("name") == npm_package(), "npm package name mismatch")
    require(metadata.get("version") == version, "npm package version mismatch")


def write_release_metadata(
    directory: Path,
    artifacts: list[Path],
    version: str,
    tag: str,
    commit: str,
) -> None:
    entries = [
        {"file": path.name, "sha256": sha256(path), "size": path.stat().st_size}
        for path in sorted(artifacts)
    ]
    (directory / "SHA256SUMS").write_text(
        "".join(f"{entry['sha256']}  {entry['file']}\n" for entry in entries),
        encoding="utf-8",
    )
    manifest = {
        "schema": 1,
        "version": version,
        "tag": tag,
        "commit": commit,
        "artifacts": entries,
    }
    (directory / "release-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    created = commit_created(commit)
    files = [
        {
            "SPDXID": f"SPDXRef-Artifact-{index}",
            "fileName": entry["file"],
            "checksums": [{"algorithm": "SHA256", "checksumValue": entry["sha256"]}],
            "licenseConcluded": "NOASSERTION",
            "copyrightText": "NOASSERTION",
        }
        for index, entry in enumerate(entries, start=1)
    ]
    sbom = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"kaleidorg-swap-sdk-{version}-release-artifacts",
        "documentNamespace": (
            "https://github.com/kaleidoswap/kaleidoswap-sdk/"
            f"releases/tag/{tag}/spdx/{commit}"
        ),
        "creationInfo": {
            "created": created,
            "creators": ["Tool: kaleidorg-swap-sdk/scripts/assemble_release.py"],
        },
        "files": files,
        "relationships": [
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": file["SPDXID"],
            }
            for file in files
        ],
    }
    (directory / "release.spdx.json").write_text(
        json.dumps(sbom, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    try:
        artifacts = collect_artifacts(args.directory, args.version)
        write_release_metadata(
            args.directory, artifacts, args.version, args.tag, args.commit
        )
    except (
        OSError,
        subprocess.CalledProcessError,
        tarfile.TarError,
        ValueError,
    ) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"Validated and described {len(artifacts)} release artifacts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
