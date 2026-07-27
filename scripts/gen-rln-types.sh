#!/usr/bin/env bash
# Generate the RGB Lightning Node (RLN) Rust types from the OpenAPI 3.1 spec.
#
# Mirrors the kaleido-sdk approach (generate TYPES only; the client is
# hand-written): OpenAPI 3.1 `components.schemas` are JSON Schema 2020-12, so we
# lift them into a `$defs` document and run typify over it.
#
# Requires: uv, cargo-typify 0.7.0.
# Run with: make generate-rln-types
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPEC="$ROOT_DIR/specs/rgb-lightning-node.yaml"
OUT="$ROOT_DIR/rln-client/src/types.rs"
CARGO_TYPIFY_VERSION="${CARGO_TYPIFY_VERSION:-0.7.0}"

command -v cargo-typify >/dev/null 2>&1 || {
  echo "❌ cargo-typify not found. Install with: cargo install cargo-typify --version ${CARGO_TYPIFY_VERSION} --locked" >&2
  exit 1
}

command -v uv >/dev/null 2>&1 || {
  echo "❌ uv not found" >&2
  exit 1
}

installed_typify_version="$(cargo typify --version | awk '{print $2}')"
if [[ "$installed_typify_version" != "$CARGO_TYPIFY_VERSION" ]]; then
  echo "❌ cargo-typify ${CARGO_TYPIFY_VERSION} is required; found ${installed_typify_version}" >&2
  exit 1
fi

TMP="$(mktemp -t rln-schema-XXXXXX.json)"
trap 'rm -f "$TMP"' EXIT

echo "→ Extracting components.schemas → JSON Schema 2020-12"
uv run --project "$ROOT_DIR/bindings/python" --locked \
  python "$ROOT_DIR/scripts/openapi_schemas_to_jsonschema.py" "$SPEC" "$TMP"

echo "→ Running typify"
cargo typify --output "$OUT" "$TMP"

echo "→ Prepending do-not-edit banner"
BANNER='// AUTO-GENERATED FILE — DO NOT EDIT MANUALLY.
//
// RGB Lightning Node (RLN) types, generated from specs/rgb-lightning-node.yaml
// via typify. Re-generate with: make generate-rln-types
'
TMP_RS="$(mktemp -t rln-types-XXXXXX.rs)"
printf '%s\n' "$BANNER" | cat - "$OUT" > "$TMP_RS" && mv "$TMP_RS" "$OUT"

echo "→ Formatting"
cargo fmt -p rln-client || true

echo "✅ Wrote $(grep -cE '^\s*pub (struct|enum) ' "$OUT") types → ${OUT#"$ROOT_DIR"/}"
