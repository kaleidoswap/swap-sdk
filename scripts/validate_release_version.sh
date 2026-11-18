#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAG_INPUT="${1:-${GITHUB_REF_NAME:-${CI_COMMIT_TAG:-}}}"

if [[ -z "${TAG_INPUT}" ]]; then
    echo "Release tag is required" >&2
    exit 1
fi

exec python3 "${ROOT_DIR}/scripts/release_version.py" validate-tag "${TAG_INPUT}"
