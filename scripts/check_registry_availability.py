#!/usr/bin/env python3
"""Fail unless the npm release version is unused and public PyPI is disabled."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request


def version_url(registry: str, package: str, version: str) -> str:
    encoded = urllib.parse.quote(package, safe="")
    return f"{registry.rstrip('/')}/{encoded}/{version}"


def require_version_available(registry: str, package: str, version: str) -> None:
    url = version_url(registry, package, version)
    request = urllib.request.Request(
        url,
        headers={"Accept": "application/json", "User-Agent": "kaleidoswap-release"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            print(f"npm version is available: {package}@{version}")
            return
        raise ValueError(
            f"npm registry returned HTTP {error.code} for {url}"
        ) from error
    except (OSError, urllib.error.URLError) as error:
        raise ValueError(
            f"could not verify npm registry availability: {error}"
        ) from error
    published = payload.get("version", version)
    raise ValueError(f"npm version already exists: {package}@{published}")


def validate_configuration() -> None:
    if os.environ.get("PYPI_PUBLISH_ENABLED", "").lower() != "false":
        raise ValueError(
            "public PyPI publishing must remain explicitly disabled until the "
            "name/version collision is resolved"
        )
    if os.environ.get("NPM_PUBLISH_ENABLED", "").lower() != "false":
        raise ValueError(
            "Phase 3 must not enable npm publishing before trusted publishing "
            "and release approval are configured"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--npm-package", default="@kaleidoswap/sdk")
    parser.add_argument("--npm-registry", default="https://registry.npmjs.org")
    args = parser.parse_args()
    try:
        validate_configuration()
        require_version_available(args.npm_registry, args.npm_package, args.version)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
