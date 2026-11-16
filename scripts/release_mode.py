#!/usr/bin/env python3
"""Decide whether a pushed release tag should build and publish, or only verify.

A version reaches npm in one of two ways. The public repository's own release
workflow builds and publishes it, or a private mirror publishes it as a security
release and pushes the tag here afterwards. Both end with a `vX.Y.Z` tag in
this repository, so the tag alone cannot say which happened. The registry can:

- the core npm package does not have this version yet: the tag starts a normal
  release (`release`);
- it already has it: the packages came from somewhere else, and the only safe
  thing left to do is check them against the release manifest (`verify`).

Getting this wrong is harmless in the direction it can go wrong. A tag for an
already-published version used to fail at preflight; it now verifies instead,
and verification publishes nothing.
"""

from __future__ import annotations

import argparse
import sys
import time
import urllib.error
import urllib.request

from check_registry_availability import version_url
from release_metadata import npm_package

NPM_REGISTRY = "https://registry.npmjs.org"
USER_AGENT = "kaleidoswap-release"
ATTEMPTS = 5
DELAY = 5.0


def published(
    version: str,
    *,
    package: str,
    registry: str = NPM_REGISTRY,
    attempts: int = ATTEMPTS,
    delay: float = DELAY,
    opener=urllib.request.urlopen,
    sleep=time.sleep,
) -> bool:
    """Whether `package@version` exists on the registry.

    A 404 is an answer, not a failure. Anything that is neither 200 nor 404 is
    retried, and if it never resolves the caller gets an error rather than a
    guess: guessing `release` on an outage would start a build that preflight
    then rejects, and guessing `verify` would skip a real release.
    """
    url = version_url(registry, package, version)
    request = urllib.request.Request(
        url, headers={"Accept": "application/json", "User-Agent": USER_AGENT}
    )
    last_error: Exception | None = None
    for attempt in range(1, attempts + 1):
        try:
            with opener(request, timeout=20):
                return True
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return False
            last_error = error
        except (OSError, urllib.error.URLError) as error:
            last_error = error
        if attempt < attempts:
            sleep(delay * attempt)
    raise ValueError(f"could not read {url}: {last_error}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--package", default=None)
    parser.add_argument("--registry", default=NPM_REGISTRY)
    args = parser.parse_args()
    try:
        package = args.package or npm_package()
        state = "published" if published(
            args.version, package=package, registry=args.registry
        ) else "unpublished"
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(state)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
