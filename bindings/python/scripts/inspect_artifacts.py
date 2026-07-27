#!/usr/bin/env python3
"""Fail when Python release archives violate the SDK packaging contract."""

from __future__ import annotations

import argparse
import tarfile
from pathlib import Path
from zipfile import ZipFile

NATIVE_SUFFIXES = (".so", ".dylib", ".dll", ".pyd")
FORBIDDEN_SDIST_PARTS = (
    "/.env",
    "/.github/",
    "/regtest/",
    "elements.cookie",
    "seed.dat",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def inspect_wheel(path: Path) -> None:
    require(not path.name.endswith("-any.whl"), f"{path.name} is a universal wheel")
    with ZipFile(path) as archive:
        names = archive.namelist()

    require(
        any(name.endswith("/kaleidoswap_sdk.py") for name in names),
        f"{path.name} is missing generated UniFFI bindings",
    )
    require(
        any(name.endswith(NATIVE_SUFFIXES) for name in names),
        f"{path.name} is missing its native library",
    )
    require(
        "kaleidoswap_sdk/rln_types.py" in names,
        f"{path.name} is missing package-local RLN models",
    )
    require(
        any(".dist-info/licenses/" in name for name in names),
        f"{path.name} is missing its license",
    )
    print(f"validated wheel: {path.name}")


def inspect_sdist(path: Path) -> None:
    with tarfile.open(path) as archive:
        names = archive.getnames()

    require(
        not any(name.endswith(NATIVE_SUFFIXES) for name in names),
        f"{path.name} contains a prebuilt native library",
    )
    forbidden = [
        name for name in names if any(part in name for part in FORBIDDEN_SDIST_PARTS)
    ]
    require(not forbidden, f"{path.name} contains forbidden files: {forbidden}")

    required_suffixes = (
        "/Cargo.lock",
        "/LICENSE",
        "/bindings/Cargo.toml",
        "/bindings/uniffi.toml",
        "/kaleidoswap_sdk/rln_types.py",
        "/macros/Cargo.toml",
        "/pyproject.toml",
        "/rln-client/Cargo.toml",
    )
    for suffix in required_suffixes:
        require(
            any(name.endswith(suffix) for name in names),
            f"{path.name} is missing {suffix}",
        )
    print(f"validated sdist: {path.name}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("dist", nargs="?", type=Path, default=Path("dist"))
    args = parser.parse_args()

    wheels = sorted(args.dist.glob("*.whl"))
    sdists = sorted(args.dist.glob("*.tar.gz"))
    require(bool(wheels), f"no wheels found in {args.dist}")
    require(bool(sdists), f"no source distributions found in {args.dist}")

    for wheel in wheels:
        inspect_wheel(wheel)
    for sdist in sdists:
        inspect_sdist(sdist)


if __name__ == "__main__":
    main()
