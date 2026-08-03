#!/usr/bin/env python3
"""Download published SDK artifacts and match them to the sealed release bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from release_metadata import LINUX_X86_64_WHEEL, npm_package, npm_tarball_name

NPM_REGISTRY = "https://registry.npmjs.org"
PYTHON_PACKAGE = "kaleidorg_swap_sdk"
PYPI_REGISTRY = "https://pypi.org/pypi"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def request_json(url: str, attempts: int, delay: float) -> dict:
    for attempt in range(1, attempts + 1):
        request = urllib.request.Request(
            url,
            headers={
                "Accept": "application/json",
                "User-Agent": "kaleidoswap-release-verifier",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = json.load(response)
            if not isinstance(payload, dict):
                raise ValueError(f"registry response is not an object: {url}")
            return payload
        except (OSError, urllib.error.URLError, json.JSONDecodeError) as error:
            if attempt == attempts:
                raise ValueError(
                    f"registry metadata was unavailable after {attempts} attempts: {url}"
                ) from error
            time.sleep(delay)
    raise AssertionError("unreachable")


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(
        url, headers={"User-Agent": "kaleidoswap-release-verifier"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        destination.write_bytes(response.read())


def load_manifest(bundle: Path, version: str) -> dict[str, dict]:
    with (bundle / "release-manifest.json").open(encoding="utf-8") as file:
        manifest = json.load(file)
    if manifest.get("version") != version:
        raise ValueError("release manifest version mismatch")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list):
        raise ValueError("release manifest artifacts must be a list")
    entries = {entry["file"]: entry for entry in artifacts}
    if len(entries) != len(artifacts):
        raise ValueError("release manifest artifact names must be unique")
    return entries


def verify_download(path: Path, expected: dict) -> None:
    if path.stat().st_size != expected.get("size"):
        raise ValueError(f"published artifact size mismatch: {path.name}")
    if sha256(path) != expected.get("sha256"):
        raise ValueError(f"published artifact checksum mismatch: {path.name}")


def npm_metadata_url(registry: str, package: str, version: str) -> str:
    encoded = urllib.parse.quote(package, safe="")
    return f"{registry.rstrip('/')}/{encoded}/{version}"


def download_npm(
    entries: dict[str, dict],
    output: Path,
    version: str,
    *,
    registry: str,
    attempts: int,
    delay: float,
) -> Path:
    expected_name = npm_tarball_name(version)
    expected = entries.get(expected_name)
    if expected is None:
        raise ValueError(f"release manifest has no npm artifact: {expected_name}")
    metadata = request_json(
        npm_metadata_url(registry, npm_package(), version), attempts, delay
    )
    if metadata.get("name") != npm_package() or metadata.get("version") != version:
        raise ValueError("npm registry package identity mismatch")
    tarball_url = metadata.get("dist", {}).get("tarball")
    if not isinstance(tarball_url, str):
        raise ValueError("npm registry metadata has no tarball URL")
    destination = output / expected_name
    download(tarball_url, destination)
    verify_download(destination, expected)
    print(f"Verified published npm artifact: {destination.name}")
    return destination


def pypi_metadata_url(registry: str, package: str, version: str) -> str:
    encoded = urllib.parse.quote(package, safe="")
    return f"{registry.rstrip('/')}/{encoded}/{version}/json"


def download_python_index(
    entries: dict[str, dict],
    output: Path,
    version: str,
    *,
    registry: str,
    attempts: int,
    delay: float,
) -> tuple[Path, Path]:
    expected = {
        name: entry
        for name, entry in entries.items()
        if name.endswith(".whl") or name.endswith(".tar.gz")
    }
    metadata = request_json(
        pypi_metadata_url(registry, PYTHON_PACKAGE, version), attempts, delay
    )
    info = metadata.get("info", {})
    if info.get("version") != version:
        raise ValueError("PyPI package version mismatch")
    urls = metadata.get("urls")
    if not isinstance(urls, list):
        raise ValueError("PyPI registry metadata has no artifact list")
    published = {entry.get("filename"): entry for entry in urls}
    if set(published) != set(expected):
        raise ValueError("PyPI artifact inventory does not match release manifest")
    for name, expected_entry in expected.items():
        digest = published[name].get("digests", {}).get("sha256")
        if digest != expected_entry.get("sha256"):
            raise ValueError(f"PyPI checksum mismatch: {name}")

    selected_names = [
        name
        for name in expected
        if LINUX_X86_64_WHEEL.search(name) or name.endswith(".tar.gz")
    ]
    if len(selected_names) != 2:
        raise ValueError("could not select Linux wheel and sdist from release manifest")

    destinations: list[Path] = []
    for name in sorted(selected_names):
        url = published[name].get("url")
        if not isinstance(url, str):
            raise ValueError(f"PyPI artifact has no download URL: {name}")
        destination = output / name
        download(url, destination)
        verify_download(destination, expected[name])
        destinations.append(destination)
        print(f"Verified published PyPI artifact: {destination.name}")
    wheel = next(path for path in destinations if path.suffix == ".whl")
    sdist = next(path for path in destinations if path.name.endswith(".tar.gz"))
    return wheel, sdist


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--npm", action="store_true")
    parser.add_argument("--pypi", action="store_true")
    parser.add_argument("--attempts", type=int, default=12)
    parser.add_argument("--delay", type=float, default=10)
    parser.add_argument("--npm-registry", default=NPM_REGISTRY)
    parser.add_argument("--pypi-registry", default=PYPI_REGISTRY)
    args = parser.parse_args()
    try:
        if args.npm == args.pypi:
            raise ValueError("select exactly one of --npm or --pypi")
        if args.attempts < 1 or args.delay < 0:
            raise ValueError("attempts must be positive and delay cannot be negative")
        args.output.mkdir(parents=True, exist_ok=False)
        entries = load_manifest(args.bundle, args.version)
        if args.npm:
            download_npm(
                entries,
                args.output,
                args.version,
                registry=args.npm_registry,
                attempts=args.attempts,
                delay=args.delay,
            )
        else:
            download_python_index(
                entries,
                args.output,
                args.version,
                registry=args.pypi_registry,
                attempts=args.attempts,
                delay=args.delay,
            )
    except (
        OSError,
        urllib.error.URLError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
