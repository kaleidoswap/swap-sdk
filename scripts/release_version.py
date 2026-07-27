#!/usr/bin/env python3
"""Synchronize and validate the public KaleidoSwap SDK versions."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


def load_toml(path: Path) -> dict:
    with path.open("rb") as file:
        return tomllib.load(file)


def normalized_name(value: str) -> str:
    return re.sub(r"[-_.]+", "-", value).lower()


def package_version(lock: dict, name: str, path: Path) -> str:
    expected_name = normalized_name(name)
    matches = [
        package["version"]
        for package in lock.get("package", [])
        if normalized_name(package.get("name", "")) == expected_name
    ]
    if len(matches) != 1:
        raise ValueError(
            f"expected one {name!r} package in {path.relative_to(ROOT)}, "
            f"found {len(matches)}"
        )
    return matches[0]


def versions() -> dict[str, str]:
    cargo_toml = load_toml(ROOT / "Cargo.toml")
    cargo_lock = load_toml(ROOT / "Cargo.lock")
    python_toml = load_toml(ROOT / "bindings/python/pyproject.toml")
    python_lock = load_toml(ROOT / "bindings/python/uv.lock")
    with (ROOT / "typescript-sdk/package.json").open(encoding="utf-8") as file:
        typescript_package = json.load(file)
    with (ROOT / "typescript-sdk/package-lock.json").open(encoding="utf-8") as file:
        typescript_lock = json.load(file)

    return {
        "Rust package": cargo_toml["package"]["version"],
        "Rust lockfile": package_version(
            cargo_lock, "kaleidoswap-sdk", ROOT / "Cargo.lock"
        ),
        "Python package": python_toml["project"]["version"],
        "Python lockfile": package_version(
            python_lock, "kaleidoswap-sdk", ROOT / "bindings/python/uv.lock"
        ),
        "TypeScript package": typescript_package["version"],
        "TypeScript lockfile": typescript_lock["packages"][""]["version"],
    }


def validate(expected: str | None = None) -> str:
    discovered = versions()
    unique_versions = set(discovered.values())
    if len(unique_versions) != 1:
        details = "\n".join(
            f"  {component}: {version}"
            for component, version in discovered.items()
        )
        raise ValueError(f"public SDK versions do not match:\n{details}")

    version = unique_versions.pop()
    if not VERSION_PATTERN.fullmatch(version):
        raise ValueError(
            f"SDK version must be stable SemVer X.Y.Z, found {version!r}"
        )
    if expected is not None and version != expected:
        raise ValueError(
            f"SDK version {version} does not match expected version {expected}"
        )
    return version


def replace_section_version(path: Path, section: str, version: str) -> None:
    contents = path.read_text(encoding="utf-8")
    section_pattern = re.compile(
        rf"(?ms)(^\[{re.escape(section)}\]\s*.*?^version\s*=\s*\")[^\"]+(\".*?$)"
    )
    updated, count = section_pattern.subn(rf"\g<1>{version}\g<2>", contents, count=1)
    if count != 1:
        raise ValueError(
            f"could not update [{section}] version in {path.relative_to(ROOT)}"
        )
    path.write_text(updated, encoding="utf-8")


def replace_lock_package_version(path: Path, name: str, version: str) -> None:
    contents = path.read_text(encoding="utf-8")
    package_pattern = re.compile(
        rf'(?ms)(^\[\[package\]\]\s*^name\s*=\s*"{re.escape(name)}"\s*'
        rf'^version\s*=\s*")[^"]+(")'
    )
    updated, count = package_pattern.subn(
        rf"\g<1>{version}\g<2>", contents, count=1
    )
    if count != 1:
        raise ValueError(
            f"could not update {name!r} in {path.relative_to(ROOT)}"
        )
    path.write_text(updated, encoding="utf-8")


def replace_json_versions(path: Path, version: str, expected_count: int) -> None:
    contents = path.read_text(encoding="utf-8")
    version_pattern = re.compile(r'(?m)(^\s*"version": ")[^"]+(",?$)')
    updated, count = version_pattern.subn(
        rf"\g<1>{version}\g<2>", contents, count=expected_count
    )
    if count != expected_count:
        raise ValueError(
            f"expected {expected_count} version fields in "
            f"{path.relative_to(ROOT)}, found {count}"
        )
    path.write_text(updated, encoding="utf-8")


def sync(version: str) -> None:
    if not VERSION_PATTERN.fullmatch(version):
        raise ValueError(
            f"release version must be stable SemVer X.Y.Z, found {version!r}"
        )

    replace_section_version(ROOT / "Cargo.toml", "package", version)
    replace_lock_package_version(
        ROOT / "Cargo.lock", "kaleidoswap-sdk", version
    )
    replace_section_version(
        ROOT / "bindings/python/pyproject.toml", "project", version
    )
    replace_lock_package_version(
        ROOT / "bindings/python/uv.lock", "kaleidoswap-sdk", version
    )

    replace_json_versions(ROOT / "typescript-sdk/package.json", version, 1)
    # npm lockfile v3 repeats the root package version under packages[""].
    replace_json_versions(ROOT / "typescript-sdk/package-lock.json", version, 2)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("show")
    subparsers.add_parser("validate")

    validate_tag = subparsers.add_parser("validate-tag")
    validate_tag.add_argument("tag")

    sync_parser = subparsers.add_parser("sync")
    sync_parser.add_argument("version")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "show":
            for component, version in versions().items():
                print(f"{component}: {version}")
        elif args.command == "validate":
            version = validate()
            print(f"Validated public SDK version {version}")
        elif args.command == "validate-tag":
            if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", args.tag):
                raise ValueError(
                    f"release tag must use the format vX.Y.Z, found {args.tag!r}"
                )
            version = validate(args.tag[1:])
            print(f"Validated release tag v{version}")
        elif args.command == "sync":
            sync(args.version)
            version = validate(args.version)
            print(f"Synchronized public SDK version {version}")
    except (KeyError, OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
