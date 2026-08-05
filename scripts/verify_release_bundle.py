#!/usr/bin/env python3
"""Verify a release bundle without rebuilding or publishing its artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from release_metadata import METADATA_FILES, PACKAGE_COUNT, RELEASE_ASSET_COUNT


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as file:
        value = json.load(file)
    require(isinstance(value, dict), f"{path.name} must contain a JSON object")
    return value


def checksum_entries(path: Path) -> dict[str, str]:
    entries: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        require(separator == "  " and digest and name, f"invalid checksum line: {line}")
        require(name not in entries, f"duplicate checksum entry: {name}")
        entries[name] = digest
    return entries


def verify(
    directory: Path,
    *,
    version: str,
    tag: str,
    commit: str,
) -> None:
    require(directory.is_dir(), f"release bundle directory not found: {directory}")
    files = {path.name: path for path in directory.iterdir() if path.is_file()}
    require(
        len(files) == RELEASE_ASSET_COUNT,
        f"expected {RELEASE_ASSET_COUNT} release assets, found {len(files)}",
    )
    require(
        METADATA_FILES <= files.keys(),
        f"release bundle is missing metadata: {sorted(METADATA_FILES - files.keys())}",
    )

    manifest = load_json(files["release-manifest.json"])
    require(manifest.get("schema") == 1, "release manifest schema mismatch")
    require(manifest.get("version") == version, "release manifest version mismatch")
    require(manifest.get("tag") == tag, "release manifest tag mismatch")
    require(manifest.get("commit") == commit, "release manifest commit mismatch")
    artifacts = manifest.get("artifacts")
    require(isinstance(artifacts, list), "release manifest artifacts must be a list")
    require(
        len(artifacts) == PACKAGE_COUNT,
        f"expected {PACKAGE_COUNT} manifest artifacts, found {len(artifacts)}",
    )

    artifact_names: set[str] = set()
    for entry in artifacts:
        require(isinstance(entry, dict), "release manifest entry must be an object")
        name = entry.get("file")
        require(
            isinstance(name, str) and Path(name).name == name,
            "release manifest artifact name must be a plain filename",
        )
        artifact_names.add(name)
    require(
        len(artifact_names) == PACKAGE_COUNT,
        "release manifest artifact names must be unique strings",
    )
    require(
        set(files) == artifact_names | METADATA_FILES,
        "release bundle files do not match manifest and metadata inventory",
    )

    checksums = checksum_entries(files["SHA256SUMS"])
    require(
        set(checksums) == artifact_names,
        "SHA256SUMS inventory does not match release manifest",
    )
    manifest_entries = {entry["file"]: entry for entry in artifacts}
    for name in sorted(artifact_names):
        path = files[name]
        digest = sha256(path)
        entry = manifest_entries[name]
        require(checksums[name] == digest, f"checksum mismatch for {name}")
        require(entry.get("sha256") == digest, f"manifest checksum mismatch for {name}")
        require(
            entry.get("size") == path.stat().st_size,
            f"manifest size mismatch for {name}",
        )

    sbom = load_json(files["release.spdx.json"])
    require(sbom.get("spdxVersion") == "SPDX-2.3", "release SBOM version mismatch")
    require(
        sbom.get("name") == f"kaleidorg-swap-sdk-{version}-release-artifacts",
        "release SBOM name/version mismatch",
    )
    require(
        sbom.get("documentNamespace")
        == (
            "https://github.com/kaleidoswap/kaleidoswap-sdk/"
            f"releases/tag/{tag}/spdx/{commit}"
        ),
        "release SBOM source identity mismatch",
    )
    sbom_files = sbom.get("files")
    require(isinstance(sbom_files, list), "release SBOM files must be a list")
    require(len(sbom_files) == PACKAGE_COUNT, "release SBOM file count mismatch")
    sbom_checksums: dict[str, str] = {}
    for entry in sbom_files:
        require(isinstance(entry, dict), "release SBOM file entry must be an object")
        checksum_values = entry.get("checksums")
        require(
            isinstance(checksum_values, list) and len(checksum_values) == 1,
            "release SBOM file must have one checksum",
        )
        checksum = checksum_values[0]
        require(
            checksum.get("algorithm") == "SHA256",
            "release SBOM checksum must use SHA256",
        )
        name = entry.get("fileName")
        value = checksum.get("checksumValue")
        require(
            isinstance(name, str) and isinstance(value, str),
            "release SBOM filename and checksum must be strings",
        )
        require(name not in sbom_checksums, f"duplicate release SBOM file: {name}")
        sbom_checksums[name] = value
    require(sbom_checksums == checksums, "release SBOM checksums do not match")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    try:
        verify(
            args.directory,
            version=args.version,
            tag=args.tag,
            commit=args.commit,
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"Verified {PACKAGE_COUNT} packages and "
        f"{RELEASE_ASSET_COUNT} intended GitHub release assets"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
