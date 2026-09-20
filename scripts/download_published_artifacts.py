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

from release_metadata import (
    LINUX_X86_64_WHEEL,
    NATIVE_ARCHIVES,
    NPM_PACKAGE_COUNT,
    PACKAGE_COUNT,
    npm_package,
    npm_tarball_name,
    react_native_npm_package,
)

NPM_REGISTRY = "https://registry.npmjs.org"
PYTHON_PACKAGE = "kaleidorg_swap_sdk"
PYPI_REGISTRY = "https://pypi.org/pypi"

# npm answers a freshly published version with 404 until it has finished
# processing the upload -- `npm publish` says so itself ("may take a few minutes
# to become available"), and how long it takes scales with the tarball. The
# 0.8.0 release was verified with a flat 12 x 10s budget, gave up 113s after a
# publish that was otherwise perfect, and took the GitHub release down with it,
# so this budget is set well past any propagation delay seen so far: eleven
# attempts backing off 10s -> 60s, about eight minutes in total.
NPM_ATTEMPTS = 11
NPM_DELAY = 10.0
NPM_MAX_DELAY = 60.0
# PyPI serves its JSON API from the upload transaction, so a version that just
# published reads back immediately. This budget covers a transient fault on the
# way to the registry, not propagation.
PYPI_ATTEMPTS = 8
PYPI_DELAY = 5.0
PYPI_MAX_DELAY = 20.0

# 404 is the propagation case above. The rest are the registry asking to be
# tried again rather than reporting anything about this release; every other
# status (401, 403, 451 ...) describes a problem that will still be true in
# eight minutes, so waiting out the budget on one only hides it.
RETRYABLE_STATUSES = frozenset({404, 408, 429})


def retryable_status(code: int) -> bool:
    return code in RETRYABLE_STATUSES or 500 <= code < 600


def backoff(delay: float, max_delay: float, attempts: int) -> list[float]:
    """The pause before each retry: `delay`, doubling, capped at `max_delay`."""
    pauses: list[float] = []
    pause = delay
    for _ in range(max(attempts - 1, 0)):
        pauses.append(pause)
        pause = min(pause * 2, max_delay)
    return pauses


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def request_json(
    url: str,
    attempts: int,
    delay: float,
    *,
    max_delay: float | None = None,
) -> dict:
    """Read registry JSON, waiting out a version that has yet to propagate.

    Every attempt reports what it got. The failure this replaces said only that
    metadata "was unavailable", which reads the same whether the version had not
    propagated yet, the registry was down, or the credential was wrong -- and
    diagnosing the 0.8.0 release meant going to the publisher's log to tell them
    apart.
    """
    pauses = backoff(delay, delay if max_delay is None else max_delay, attempts)
    waited = 0.0
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
            if attempt > 1:
                print(
                    f"Registry metadata arrived after {waited:.0f}s of backoff: {url}",
                    file=sys.stderr,
                )
            return payload
        except urllib.error.HTTPError as error:
            reason = f"HTTP {error.code}"
            fatal = not retryable_status(error.code)
            failure: Exception = error
        except (OSError, urllib.error.URLError, json.JSONDecodeError) as error:
            reason = str(error)
            fatal = False
            failure = error
        if fatal:
            raise ValueError(f"registry returned {reason} for {url}") from failure
        if attempt == attempts:
            raise ValueError(
                f"registry metadata was unavailable after {attempts} attempts "
                f"over {waited:.0f}s of backoff, last {reason}: {url}"
            ) from failure
        pause = pauses[attempt - 1]
        print(
            f"Waiting for registry metadata ({reason}, attempt {attempt}/{attempts}), "
            f"retrying in {pause:.0f}s: {url}",
            file=sys.stderr,
        )
        time.sleep(pause)
        waited += pause
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
    max_delay: float | None = None,
    package: str | None = None,
) -> Path:
    package = npm_package() if package is None else package
    expected_name = npm_tarball_name(version, package)
    expected = entries.get(expected_name)
    if expected is None:
        raise ValueError(f"release manifest has no npm artifact: {expected_name}")
    metadata = request_json(
        npm_metadata_url(registry, package, version),
        attempts,
        delay,
        max_delay=max_delay,
    )
    if metadata.get("name") != package or metadata.get("version") != version:
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
    max_delay: float | None = None,
) -> tuple[Path, Path]:
    expected = {
        name: entry
        for name, entry in entries.items()
        if name.endswith(".whl") or name.endswith(".tar.gz")
    }
    metadata = request_json(
        pypi_metadata_url(registry, PYTHON_PACKAGE, version),
        attempts,
        delay,
        max_delay=max_delay,
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

    # Download and hash *every* artifact. The digest comparison above trusts
    # PyPI's self-reported metadata, so a registry or intermediary serving
    # correct metadata with tampered bytes would pass it. Only re-hashing the
    # downloaded file proves the published bytes are the sealed bytes.
    destinations: list[Path] = []
    for name in sorted(expected):
        url = published[name].get("url")
        if not isinstance(url, str):
            raise ValueError(f"PyPI artifact has no download URL: {name}")
        destination = output / name
        download(url, destination)
        verify_download(destination, expected[name])
        destinations.append(destination)
        print(f"Verified published PyPI artifact: {destination.name}")
    python_count = PACKAGE_COUNT - NPM_PACKAGE_COUNT - len(NATIVE_ARCHIVES) - 1
    if len(destinations) != python_count:
        raise ValueError(
            f"expected {python_count} Python artifacts, "
            f"byte-verified {len(destinations)}"
        )

    # The smoke tests need one installable wheel for this runner plus the sdist.
    wheels = [path for path in destinations if LINUX_X86_64_WHEEL.search(path.name)]
    sdists = [path for path in destinations if path.name.endswith(".tar.gz")]
    if len(wheels) != 1 or len(sdists) != 1:
        raise ValueError(
            "could not select exactly one Linux wheel and one sdist for the "
            f"smoke tests (wheels={len(wheels)}, sdists={len(sdists)})"
        )
    return wheels[0], sdists[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--npm", action="store_true")
    parser.add_argument("--pypi", action="store_true")
    # The two registries propagate on different scales, so neither default is
    # shared. Left unset, each side gets the budget its own registry needs.
    parser.add_argument("--attempts", type=int)
    parser.add_argument("--delay", type=float)
    parser.add_argument("--max-delay", type=float)
    parser.add_argument("--npm-registry", default=NPM_REGISTRY)
    parser.add_argument("--pypi-registry", default=PYPI_REGISTRY)
    args = parser.parse_args()
    try:
        if args.npm == args.pypi:
            raise ValueError("select exactly one of --npm or --pypi")
        if args.npm:
            attempts, delay, max_delay = NPM_ATTEMPTS, NPM_DELAY, NPM_MAX_DELAY
        else:
            attempts, delay, max_delay = PYPI_ATTEMPTS, PYPI_DELAY, PYPI_MAX_DELAY
        attempts = attempts if args.attempts is None else args.attempts
        delay = delay if args.delay is None else args.delay
        max_delay = max_delay if args.max_delay is None else args.max_delay
        if attempts < 1 or delay < 0:
            raise ValueError("attempts must be positive and delay cannot be negative")
        if max_delay < delay:
            raise ValueError("max delay cannot be shorter than the first delay")
        args.output.mkdir(parents=True, exist_ok=False)
        entries = load_manifest(args.bundle, args.version)
        if args.npm:
            for package in (npm_package(), react_native_npm_package()):
                download_npm(
                    entries,
                    args.output,
                    args.version,
                    registry=args.npm_registry,
                    attempts=attempts,
                    delay=delay,
                    max_delay=max_delay,
                    package=package,
                )
        else:
            download_python_index(
                entries,
                args.output,
                args.version,
                registry=args.pypi_registry,
                attempts=attempts,
                delay=delay,
                max_delay=max_delay,
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
