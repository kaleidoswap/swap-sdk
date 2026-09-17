#!/bin/sh
# Fetch the prebuilt native binaries for this package version.
#
# The .so and .a files are tens of megabytes per architecture, which does not
# belong in an npm tarball that every install downloads in full. They are
# attached to this version's GitHub release instead, and fetched here.
#
# The alternative — building from source on install — would put a Rust
# toolchain, the Android NDK and Xcode in the critical path of `npm install`
# for every partner. That is the thing this package exists to avoid.
set -eu

REPO="https://github.com/kaleidoswap/swap-sdk"
VERSION="$(node -p "require('./package.json').version")"
TAG="v${VERSION}"

# A checkout of this repository builds the binaries with `npm run ubrn:build`
# and has no release to fetch from. Only a published install does.
if [ -f ../../Cargo.toml ]; then
  echo "postinstall: in-repo checkout, skipping prebuilt download" >&2
  exit 0
fi

fetch() {
  archive="$1"
  url="${REPO}/releases/download/${TAG}/${archive}"
  if ! curl -fsSL "$url" --output "$archive"; then
    echo "postinstall: could not download ${url}" >&2
    echo "postinstall: build from source with 'npm run ubrn:build' in a checkout of ${REPO}" >&2
    exit 1
  fi
  unzip -qo "$archive"
  rm -f "$archive"
}

fetch android-artifacts.zip
fetch ios-artifacts.zip
