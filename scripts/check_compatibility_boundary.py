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
import sys

from check_binding_parity import ALIASES, WASM_DIR, UNIFFI_DIR, exports

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


def masked(source: str) -> str:
    # Preserve offsets while excluding calls inside comments/string contents.
    return re.sub(
        r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/',
        lambda m: " " * len(m[0]),
        source,
        flags=re.S,
    )


def closing_paren(source: str, opening: int) -> int:
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == "(") - (source[index] == ")")
        if depth == 0:
            return index
    raise ValueError("unterminated router call")


def inventory(source: str, revision: str) -> dict:
    routes = set()

    def walk(name: str, prefix: str, stack: tuple = ()) -> None:
        require(name not in stack, f"recursive router nesting: {stack + (name,)}")
        body = function(source, name)
        clean = masked(body)
        cursor = 0
        calls = re.compile(
            r"\.\s*(route|nest|merge|route_service|nest_service|fallback|fallback_service)\s*\("
        )
        while match := calls.search(clean, cursor):
            end = closing_paren(clean, match.end() - 1)
            arguments = body[match.end() : end]
            cursor = end + 1  # Do not mistake a MethodRouter.merge for Router.merge.
            kind = match[1]
            require(
                kind in {"route", "nest"},
                f"unsupported router {kind} in {name}; review route inventory",
            )
            literal = re.fullmatch(
                r'\s*"([^"\\\n]+)"\s*,\s*(.*?)\s*,?\s*', arguments, flags=re.S
            )
            require(
                literal is not None,
                f"non-literal/unknown {kind} in {name}; review route inventory",
            )
            path, expression = literal.groups()
            require(path.startswith("/"), f"invalid router path in {name}: {path}")
            full_path = prefix.rstrip("/") + path
            if kind == "nest":
                target = re.match(r"([A-Za-z_]\w*)\s*\(", expression)
                require(
                    target is not None,
                    f"nest target is not a local function in {name}: {path}",
                )
                last = closing_paren(masked(expression), target.end() - 1)
                require(
                    not expression[last + 1 :].strip(),
                    f"unsupported nested router expression in {name}: {path}",
                )
                walk(target[1], full_path, stack + (name,))
            else:
                expression = masked(expression)
                require(
                    not re.search(r"\b(?:any|on)\s*\(", expression),
                    f"unsupported HTTP method mapping in {name}: {path}",
                )
                verbs = set(
                    re.findall(
                        r"\b(get|post|patch|delete|put|options|head|trace|connect)\s*\(",
                        expression,
                    )
                )
                require(bool(verbs), f"no HTTP method for {full_path} in {name}")
                routes.update((verb.upper(), full_path) for verb in verbs)

    walk("router", "")
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
        "routes": [list(route) for route in sorted(routes)],
    }


def load_binding_sources() -> tuple[str, str]:
    # Match the parity guard: exports may live in any recursively nested module.
    return tuple(
        "\n".join(path.read_text() for path in sorted(directory.rglob("*.rs")))
        for directory in (WASM_DIR, UNIFFI_DIR)
    )


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
    proof = helper.find("verify_submarine_preimage")
    signing = helper.find(".partial_sign(")
    require(
        proof >= 0 and signing >= 0 and proof < signing,
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
    wasm, native = load_binding_sources()
    validate(
        maker,
        (ROOT / "src/swaps/boltz.rs").read_text(),
        (ROOT / "src/swaps/wrappers.rs").read_text(),
        wasm,
        native,
    )
    if args.refresh_maker:
        SNAPSHOT.write_text(json.dumps(maker, indent=2) + "\n")
    print(
        f"Upstream-only route boundary verified against maker {maker['source']['revision']}"
    )


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
