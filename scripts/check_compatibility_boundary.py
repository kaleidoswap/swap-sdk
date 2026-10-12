#!/usr/bin/env python3
"""Check the deliberately limited upstream-only SDK compatibility boundary.

By default, check SDK exports against a pinned inventory extracted from the
maker router. Use --maker-router PATH for a fresh local maker checkout, or
--refresh-maker REF to fetch and commit a new inventory (requires gh read access).
This is a source contract check, not a live endpoint probe.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from check_binding_parity import ALIASES, exports

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = ROOT / "tests/fixtures/maker-compatibility-routes.json"
REPOSITORY = "kaleidoswap/kaleidoswap-maker-rs"
ROUTER = "crates/maker-api/src/lib.rs"
UPSTREAM_ROUTES = {
    ("GET", "/v2/swap/submarine/{id}/claim"),
    ("POST", "/v2/swap/submarine/{id}/claim"),
    ("POST", "/v2/chain/BTC/transaction"),
    ("POST", "/v2/chain/L-BTC/transaction"),
    ("POST", "/v2/lightning/BTC/bolt12"),
    ("PATCH", "/v2/lightning/BTC/bolt12"),
    ("POST", "/v2/lightning/BTC/bolt12/delete"),
    ("POST", "/v2/lightning/BTC/bolt12/fetch"),
    ("GET", "/v2/lightning/BTC/bolt12/L-BTC"),
}


def function(text: str, name: str) -> str:
    match = re.search(rf"(?m)^\s*(?:pub )?(?:async )?fn {name}\(", text)
    if match is None:
        raise ValueError(f"missing function {name}")
    # Ignore strings/comments while matching the function's closing brace, so
    # a final router function cannot accidentally absorb routes from its tests.
    masked = re.sub(
        r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/',
        lambda m: " " * len(m[0]),
        text,
        flags=re.S,
    )
    start = masked.find("{", match.end())
    if start < 0:
        raise ValueError(f"missing body for {name}")
    depth = 0
    for end in range(start, len(masked)):
        depth += (masked[end] == "{") - (masked[end] == "}")
        if depth == 0:
            return text[match.end() : end + 1]
    raise ValueError(f"unterminated body for {name}")


def inventory(source: str, revision: str) -> dict:
    routes = []
    for name, prefix in [
        ("v2_routes", "/v2"),
        ("swap_routes", "/v2/swap"),
        ("admin_routes", "/v2/admin"),
    ]:
        body = function(source, name)
        require(
            ".fallback(" not in body and ".route_service(" not in body,
            f"router layout changed in {name}; review fallback/service routes",
        )
        # The router uses literal paths in chained .route calls. Stop each
        # expression at the next chained route/nest/layer (not route_layer).
        matches = list(re.finditer(r'\.route\(\s*"([^"\n]+)"\s*,', body))
        if not matches:
            raise ValueError(f"router layout changed: no routes in {name}")
        for index, match in enumerate(matches):
            expression = body[
                match.end() : matches[index + 1].start()
                if index + 1 < len(matches)
                else len(body)
            ]
            expression = re.split(
                r"\n        \.(?:nest|layer|with_state)\(", expression
            )[0]
            expression = re.sub(r"//[^\n]*", "", expression)
            verbs = set(re.findall(r"\b(get|post|patch|delete|put)\(", expression))
            if not verbs:
                raise ValueError(
                    f"router layout changed: no HTTP method for {prefix}{match[1]}"
                )
            routes.extend([verb.upper(), prefix + match[1]] for verb in sorted(verbs))
    return {
        "source": {
            "repository": REPOSITORY,
            "revision": revision,
            "file": ROUTER,
            "sha256": hashlib.sha256(source.encode()).hexdigest(),
        },
        "adminAuth": bool(
            re.search(
                r"\.route_layer\(from_fn_with_state\(state,\s*auth::require_admin_auth\)\)",
                re.sub(r"//[^\n]*", "", function(source, "admin_routes")),
            )
        ),
        "routes": sorted(routes),
    }


def registered(routes: list, method: str, path: str) -> bool:
    for verb, template in routes:
        # A parameterized route counts too; a generic /swap/{id} cannot serve
        # a longer /swap/submarine/{id}/claim path.
        pattern = "/".join(
            ".+"
            if part.startswith("{*")
            else "[^/]+"
            if part.startswith("{")
            else re.escape(part)
            for part in template.split("/")
        )
        if verb == method and re.fullmatch(pattern, path):
            return True
    return False


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(maker: dict, core: str, wrappers: str, wasm: str, native: str) -> None:
    routes = maker["routes"]
    for method, path in UPSTREAM_ROUTES:
        require(
            not registered(routes, method, path),
            f"maker now registers {method} {path}; review the support matrix and binding decision",
        )
    for path in [
        "/v2/swap/reverse/{id}/claim",
        "/v2/swap/chain/{id}/claim",
        "/v2/admin/liquid/tx/broadcast",
        "/v2/admin/node/offer",
    ]:
        require(registered(routes, "POST", path), f"maker route inventory lost {path}")
    require(
        maker["adminAuth"], "operator endpoints must remain separately authenticated"
    )

    # Preserve the existing Rust compatibility methods and the exact routes
    # they require; this decision does not remove or redirect upstream support.
    methods = {
        "get_submarine_claim_tx_details": (
            "swap/submarine/{id}/claim",
            "self.get_json",
        ),
        "post_submarine_claim_tx_details": (
            "swap/submarine/{id}/claim",
            "self.post_json",
        ),
        "broadcast_tx": ("chain/{chain}/transaction", "self.post_json"),
        "post_bolt12_offer": ("lightning/BTC/bolt12", "self.post_json"),
        "patch_bolt12_offer": ("lightning/BTC/bolt12", "self.patch"),
        "delete_bolt12_offer": ("lightning/BTC/bolt12/delete", "self.post_json"),
        "get_bolt12_invoice": ("lightning/BTC/bolt12/fetch", "self.post_json"),
        "get_bolt12_params": ("lightning/BTC/bolt12/L-BTC", "self.get_json"),
    }
    for method, (path, call) in methods.items():
        body = function(core, method)
        require(
            f'"{path}"' in body and call in body,
            f"Rust compatibility route changed: {method}",
        )
    raw_methods = set(methods)
    wasm_exports = exports(wasm, "#[wasm_bindgen]")
    native_exports = exports(native, "#[uniffi::export")
    require(
        not (raw_methods & wasm_exports.get("BoltzClient", set())),
        "raw upstream-only route added to WASM; review the support policy",
    )
    require(
        not (raw_methods & native_exports.get("SwapClient", set())),
        "raw upstream-only route added to UniFFI; review the support policy",
    )
    require(
        "submarine_cooperative_claim" in native_exports.get("SwapScript", set()),
        "existing native submarine co-sign helper disappeared",
    )
    require(
        "submarine_cooperative_claim" not in wasm_exports.get("SwapScript", set()),
        "submarine co-sign added to WASM; review the support policy",
    )
    helper = function(wrappers, "submarine_cooperative_claim")
    require(
        helper.index("verify_submarine_preimage") < helper.index(".partial_sign("),
        "submarine co-sign must retain its proof-of-payment gate",
    )
    chain = function(wasm, "construct_cooperative_claim")
    require(
        ".with_chain_claim(" in chain and ".construct_claim(" in chain,
        "WASM cooperative chain claim behavior changed",
    )
    require(
        ALIASES[("SwapScript", "construct_cooperative_claim")] == "construct_claim",
        "chain claim must not alias the distinct submarine co-sign capability",
    )
    require(".with_chain_claim(" in native, "native chain-claim options disappeared")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--maker-router", type=Path)
    parser.add_argument("--refresh-maker", metavar="REF")
    args = parser.parse_args()
    if args.refresh_maker:

        def api(endpoint):
            return subprocess.check_output(
                [
                    "gh",
                    "api",
                    f"repos/{REPOSITORY}/{endpoint}",
                    "-H",
                    "Accept: application/vnd.github.raw+json",
                ],
                text=True,
            )

        revision = json.loads(api(f"commits/{args.refresh_maker}"))["sha"]
        maker = inventory(api(f"contents/{ROUTER}?ref={revision}"), revision)
    elif args.maker_router:
        maker = inventory(args.maker_router.read_text(), "local-checkout")
    else:
        maker = json.loads(SNAPSHOT.read_text())
    native = "\n".join(
        path.read_text() for path in sorted((ROOT / "bindings/src").glob("*.rs"))
    )
    validate(
        maker,
        (ROOT / "src/swaps/boltz.rs").read_text(),
        (ROOT / "src/swaps/wrappers.rs").read_text(),
        (ROOT / "bindings-wasm/src/lib.rs").read_text(),
        native,
    )
    if args.refresh_maker:
        SNAPSHOT.write_text(json.dumps(maker, indent=2) + "\n")
    print(
        f"Upstream-only route boundary verified against maker {maker['source']['revision']}"
    )


if __name__ == "__main__":
    main()
