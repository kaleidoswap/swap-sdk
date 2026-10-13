# /// script
# requires-python = ">=3.11"
# dependencies = ["PyYAML==6.0.3"]
# ///
"""Refresh/check the authenticated maker schema used by SDK compatibility CI.

Run with `uv run scripts/update_maker_swap_schema.py [--check] [--revision REF]`.
`gh` must authenticate an account/token with read access to the private maker repo.
"""

import argparse
import json
from pathlib import Path
import subprocess

import yaml

REPOSITORY = "kaleidoswap/kaleidoswap-maker-rs"
DESTINATION = (
    Path(__file__).resolve().parents[1] / "tests/fixtures/maker-swap-schema.json"
)
PATHS = {
    "/v2/swap/submarine",
    "/v2/swap/reverse",
    "/v2/swap/chain",
    "/v2/swap/{id}",
    "/v2/swap/restore",
    "/v2/swap/restore/index",
}


def api(endpoint, raw=False):
    args = ["gh", "api", f"repos/{REPOSITORY}/{endpoint}"]
    if raw:
        args += ["-H", "Accept: application/vnd.github.raw+json"]
    return subprocess.check_output(args, text=True)


def contract(document, revision):
    refs = set()

    def shape(value):
        if isinstance(value, dict):
            if "$ref" in value:
                refs.add(value["$ref"].split("/")[-1])
            return {
                k: shape(v)
                for k, v in value.items()
                if k
                not in {
                    "description",
                    "summary",
                    "example",
                    "examples",
                    "title",
                    "externalDocs",
                }
            }
        if isinstance(value, list):
            return [shape(v) for v in value]
        return value

    routes = {}
    for path in sorted(PATHS):
        operation = document["paths"][path].get(
            "post", document["paths"][path].get("get")
        )
        request = (
            operation.get("requestBody", {})
            .get("content", {})
            .get("application/json", {})
            .get("schema")
        )
        responses = {
            status: response.get("content", {})
            .get("application/json", {})
            .get("schema")
            for status, response in operation["responses"].items()
            if str(status).startswith("2")
        }
        routes[path] = shape({"request": request, "responses": responses})
    schemas = {}
    while refs - schemas.keys():
        for name in sorted(refs - schemas.keys()):
            schemas[name] = shape(document["components"]["schemas"][name])
    return {
        "source": {
            "repository": REPOSITORY,
            "revision": revision,
            "file": "openapi.yaml",
        },
        "routes": routes,
        "schemas": dict(sorted(schemas.items())),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="main")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    revision = json.loads(api(f"commits/{args.revision}"))["sha"]
    document = yaml.safe_load(api(f"contents/openapi.yaml?ref={revision}", raw=True))
    latest = contract(document, revision)
    if args.check:
        committed = json.loads(DESTINATION.read_text())
        for section in ["routes", "schemas"]:
            if latest[section] != committed[section]:
                raise SystemExit(
                    "Maker swap schema drifted; refresh the snapshot and review DTO compatibility"
                )
        print(f"Maker swap schema unchanged at {revision}")
    else:
        DESTINATION.write_text(json.dumps(latest, indent=2) + "\n")
        print(f"Updated {DESTINATION} at {revision}")


if __name__ == "__main__":
    main()
