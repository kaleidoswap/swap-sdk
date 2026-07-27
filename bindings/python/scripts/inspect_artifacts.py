#!/usr/bin/env python3
"""Fail when Python release archives violate the SDK packaging contract."""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import io
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
        record_names = [name for name in names if name.endswith(".dist-info/RECORD")]
        require(
            len(record_names) == 1,
            f"{path.name} must contain exactly one .dist-info/RECORD",
        )
        record_name = record_names[0]
        rows = list(csv.reader(io.StringIO(archive.read(record_name).decode())))
        records = {row[0]: row[1:] for row in rows if row}
        require(
            set(names) == set(records),
            f"{path.name} RECORD does not enumerate every archive member",
        )
        for name, (digest, size) in records.items():
            if name == record_name:
                require(
                    not digest and not size,
                    f"{path.name} RECORD entry must not hash itself",
                )
                continue
            data = archive.read(name)
            actual = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(
                b"="
            )
            require(
                digest == f"sha256={actual.decode()}" and size == str(len(data)),
                f"{path.name} has an invalid RECORD entry for {name}",
            )

    require(
        "kaleidoswap_sdk/_generated_uniffi.py" in names,
        f"{path.name} is missing the package-local UniFFI fallback",
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
        "/kaleidoswap_sdk/_generated_uniffi.py",
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
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--wheel-only", action="store_true")
    mode.add_argument("--sdist-only", action="store_true")
    args = parser.parse_args()

    wheels = sorted(args.dist.glob("*.whl"))
    sdists = sorted(args.dist.glob("*.tar.gz"))
    if not args.sdist_only:
        require(bool(wheels), f"no wheels found in {args.dist}")
    if not args.wheel_only:
        require(bool(sdists), f"no source distributions found in {args.dist}")

    if not args.sdist_only:
        for wheel in wheels:
            inspect_wheel(wheel)
    if not args.wheel_only:
        for sdist in sdists:
            inspect_sdist(sdist)


if __name__ == "__main__":
    main()
