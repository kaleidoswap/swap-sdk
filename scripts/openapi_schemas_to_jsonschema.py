#!/usr/bin/env python3
"""Extract an OpenAPI 3.1 document's `components.schemas` into a standalone
JSON Schema 2020-12 document that typify can consume.

OpenAPI 3.1's schema objects *are* JSON Schema 2020-12, so the only work is:
  1. lift `components.schemas` into top-level `$defs`, and
  2. rewrite every `$ref: "#/components/schemas/X"` to `"#/$defs/X"`.

Usage: openapi_schemas_to_jsonschema.py <input openapi .yaml/.json> <output .json>
"""
import json
import sys

import yaml


def rewrite_refs(node):
    """Recursively rewrite component-schema refs to local $defs refs."""
    if isinstance(node, dict):
        return {
            k: (
                v.replace("#/components/schemas/", "#/$defs/")
                if k == "$ref" and isinstance(v, str)
                else rewrite_refs(v)
            )
            for k, v in node.items()
        }
    if isinstance(node, list):
        return [rewrite_refs(item) for item in node]
    return node


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2

    in_path, out_path = sys.argv[1], sys.argv[2]
    with open(in_path) as f:
        doc = yaml.safe_load(f)

    schemas = doc.get("components", {}).get("schemas")
    if not schemas:
        print(f"❌ no components.schemas found in {in_path}", file=sys.stderr)
        return 1

    defs = rewrite_refs(schemas)
    # typify emits a named Rust type for every entry in `$defs`, so a bare
    # definitions document is enough — no synthetic root type needed.
    out = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": defs,
    }

    with open(out_path, "w") as f:
        json.dump(out, f, indent=2)
    print(f"✔ wrote {len(defs)} schemas → {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
