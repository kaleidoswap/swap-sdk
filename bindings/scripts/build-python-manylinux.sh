#!/usr/bin/env bash
set -euo pipefail

# cp311+: the generated RLN models use enum.StrEnum (Python 3.11+), matching the
# package's requires-python >=3.11.
export PATH="/opt/python/cp311-cp311/bin:$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain none
fi

make build-release

cd python
rm -rf dist
uv build

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

auditwheel repair --plat manylinux_2_28_x86_64 dist/*-py3-none-any.whl --wheel-dir "$tmp_dir"
rm dist/*-py3-none-any.whl
mv "$tmp_dir"/*.whl dist/

python -m venv "$tmp_dir/wheel"
# Prefer the freshly-built wheel, but allow the index to resolve runtime deps
# (pydantic). `--no-index` would fail now that the package has a real dependency.
"$tmp_dir/wheel/bin/python" -m pip install --find-links dist dist/*.whl
"$tmp_dir/wheel/bin/python" -c "import kaleidoswap_sdk"

python -m venv "$tmp_dir/sdist"
"$tmp_dir/sdist/bin/python" -m pip install dist/*.tar.gz
"$tmp_dir/sdist/bin/python" -c "import kaleidoswap_sdk"
