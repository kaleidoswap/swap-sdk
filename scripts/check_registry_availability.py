#!/usr/bin/env python3
"""Validate release registry flags and ensure enabled versions are unused."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

from release_metadata import npm_package


def version_url(
    registry: str,
    package: str,
    version: str,
    *,
    json_api: bool = False,
) -> str:
    encoded = urllib.parse.quote(package, safe="")
    suffix = "/json" if json_api else ""
    return f"{registry.rstrip('/')}/{encoded}/{version}{suffix}"


def require_version_available(
    registry: str,
    package: str,
    version: str,
    registry_name: str,
    *,
    json_api: bool = False,
) -> None:
    url = version_url(registry, package, version, json_api=json_api)
    request = urllib.request.Request(
        url,
        headers={"Accept": "application/json", "User-Agent": "kaleidoswap-release"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            print(f"{registry_name} version is available: {package}@{version}")
            return
        raise ValueError(
            f"{registry_name} returned HTTP {error.code} for {url}"
        ) from error
    except (OSError, urllib.error.URLError) as error:
        raise ValueError(
            f"could not verify {registry_name} availability: {error}"
        ) from error
    published = payload.get("version", version)
    raise ValueError(f"{registry_name} version already exists: {package}@{published}")


def flag(name: str) -> bool:
    value = os.environ.get(name, "").lower()
    if value not in {"true", "false"}:
        raise ValueError(f"{name} must be explicitly set to true or false")
    return value == "true"


def validate_configuration() -> tuple[bool, bool]:
    npm_enabled = flag("NPM_PUBLISH_ENABLED")
    test_pypi_enabled = flag("TEST_PYPI_PUBLISH_ENABLED")
    if flag("PYPI_PUBLISH_ENABLED"):
        raise ValueError(
            "public PyPI publishing must remain explicitly disabled until a "
            "trusted publisher is configured for kaleidorg-swap-sdk"
        )
    return npm_enabled, test_pypi_enabled


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--npm-package", default=npm_package())
    parser.add_argument("--npm-registry", default="https://registry.npmjs.org")
    parser.add_argument("--python-package", default="kaleidorg_swap_sdk")
    parser.add_argument("--test-pypi-registry", default="https://test.pypi.org/pypi")
    parser.add_argument(
        "--check-test-pypi",
        action="store_true",
        help="check TestPyPI availability even while its publisher is disabled",
    )
    parser.add_argument(
        "--flags-only",
        action="store_true",
        help=(
            "validate publisher flags without contacting a registry; a rehearsal "
            "must not require the current version to still be unclaimed"
        ),
    )
    args = parser.parse_args()
    try:
        _, test_pypi_enabled = validate_configuration()
        if args.flags_only:
            print("Validated publisher configuration without a registry check")
            return 0
        require_version_available(
            args.npm_registry,
            args.npm_package,
            args.version,
            "npm",
        )
        if test_pypi_enabled or args.check_test_pypi:
            require_version_available(
                args.test_pypi_registry,
                args.python_package,
                args.version,
                "TestPyPI",
                json_api=True,
            )
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
