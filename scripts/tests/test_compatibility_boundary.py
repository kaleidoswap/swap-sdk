from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import check_compatibility_boundary as boundary  # noqa: E402


class CompatibilityBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.maker = json.loads(boundary.SNAPSHOT.read_text())
        self.core = (ROOT / "src/swaps/boltz.rs").read_text()
        self.wrappers = (ROOT / "src/swaps/wrappers.rs").read_text()
        self.wasm, self.native = boundary.load_binding_sources()

    def validate(self, **changes):
        values = {
            name: getattr(self, name)
            for name in ["maker", "core", "wrappers", "wasm", "native"]
        }
        boundary.validate(**(values | changes))

    def test_current_source_contract(self):
        self.validate()

    def test_every_new_public_route_requires_a_policy_review(self):
        for route in boundary.UPSTREAM_ROUTES:
            with self.subTest(route=route):
                maker = copy.deepcopy(self.maker)
                maker["routes"].append(list(route))
                with self.assertRaisesRegex(ValueError, "maker now registers"):
                    self.validate(maker=maker)

    def test_parameterized_broadcast_route_is_also_detected(self):
        maker = copy.deepcopy(self.maker)
        maker["routes"].append(["POST", "/v2/chain/{currency}/transaction"])
        with self.assertRaisesRegex(ValueError, "maker now registers"):
            self.validate(maker=maker)

    def test_operator_broadcast_does_not_imply_public_broadcast(self):
        self.assertTrue(
            boundary.registered(
                self.maker["routes"], "POST", "/v2/admin/liquid/tx/broadcast"
            )
        )
        self.assertFalse(
            boundary.registered(
                self.maker["routes"], "POST", "/v2/chain/L-BTC/transaction"
            )
        )
        maker = copy.deepcopy(self.maker)
        maker["adminAuth"] = False
        with self.assertRaisesRegex(ValueError, "separately authenticated"):
            self.validate(maker=maker)

    def test_raw_exports_require_an_explicit_support_decision(self):
        for surface, marker, name in [
            ("wasm", "#[wasm_bindgen]", "BoltzClient"),
            ("native", "#[uniffi::export]", "SwapClient"),
        ]:
            for method in [
                "broadcast_tx",
                "get_bolt12_invoice",
                "get_submarine_claim_tx_details",
            ]:
                with self.subTest(surface=surface, method=method):
                    source = (
                        getattr(self, surface)
                        + f"\n{marker}\nimpl {name} {{\n    pub async fn {method}() {{}}\n}}"
                    )
                    with self.assertRaisesRegex(
                        ValueError, "raw upstream-only route added"
                    ):
                        self.validate(**{surface: source})

    def test_rust_compatibility_routes_are_retained(self):
        with self.assertRaisesRegex(ValueError, "Rust compatibility route changed"):
            self.validate(
                core=self.core.replace(
                    '"lightning/BTC/bolt12/fetch"', '"admin/node/offer/pay"'
                )
            )

    def test_native_submarine_helper_is_retained_and_is_not_chain_claim(self):
        wasm = (
            self.wasm
            + "\n#[wasm_bindgen]\nimpl SwapScript {\n    pub async fn submarine_cooperative_claim() {}\n}"
        )
        with self.assertRaisesRegex(ValueError, "submarine co-sign added to WASM"):
            self.validate(wasm=wasm)
        with self.assertRaisesRegex(ValueError, "existing native submarine"):
            self.validate(
                native=self.native.replace(
                    "pub async fn submarine_cooperative_claim(",
                    "async fn submarine_cooperative_claim(",
                )
            )
        with mock.patch.dict(
            boundary.ALIASES,
            {
                (
                    "SwapScript",
                    "construct_cooperative_claim",
                ): "submarine_cooperative_claim"
            },
        ):
            with self.assertRaisesRegex(ValueError, "distinct submarine co-sign"):
                self.validate()

    def test_router_inventory_reads_multiline_and_combined_methods(self):
        source = self.router_fixture()
        inventory = boundary.inventory(source, "fixture")
        self.assertIn(["GET", "/v2/chain/{currency}/height"], inventory["routes"])
        self.assertIn(["POST", "/v2/chain/{currency}/height"], inventory["routes"])
        self.assertIn(["GET", "/v2/swap/submarine"], inventory["routes"])
        self.assertIn(["GET", "/v1/card"], inventory["routes"])
        self.assertTrue(inventory["adminAuth"])
        with_test_router = (
            source
            + '\n#[cfg(test)]\nmod tests { fn fixture() { Router::new().route("/fake", post(fake)); } }'
        )
        self.assertEqual(
            boundary.inventory(with_test_router, "fixture")["routes"],
            inventory["routes"],
        )
        without_auth = source.replace(
            ".route_layer(from_fn_with_state(state, auth::require_admin_auth))",
            "// auth::require_admin_auth is missing",
        )
        self.assertFalse(boundary.inventory(without_auth, "fixture")["adminAuth"])

    @staticmethod
    def router_fixture():
        return """
fn router() {
    Router::new().route("/v1/card", get(card)).nest("/v2", v2_routes())
}
fn v2_routes() {
    Router::new().route(
        "/chain/{currency}/height", get(chain::height).post(chain::update))
        .nest("/swap", swap_routes()).nest("/admin", admin_routes())
}
fn swap_routes() {
    Router::new().route("/submarine", post(swap::create).merge(get(pairs::submarine)))
}
fn admin_routes() {
    Router::new().route("/liquid/tx/broadcast", post(admin::broadcast))
        .route_layer(from_fn_with_state(state, auth::require_admin_auth))
}
"""

    def test_new_nested_bolt12_router_is_inventoried_and_rejected(self):
        source = self.router_fixture().replace(
            '.nest("/swap",', '.nest("/lightning", lightning_routes()).nest("/swap",'
        )
        source += '\nfn lightning_routes() { Router::new().route("/BTC/bolt12", post(create).patch(update)).route("/BTC/bolt12/fetch", post(fetch)) }'
        maker = boundary.inventory(source, "fixture")
        self.assertIn(["PATCH", "/v2/lightning/BTC/bolt12"], maker["routes"])
        self.assertIn(["POST", "/v2/lightning/BTC/bolt12/fetch"], maker["routes"])
        with self.assertRaisesRegex(ValueError, "maker now registers"):
            self.validate(maker=maker)

    def test_unknown_nesting_merge_and_services_fail_closed(self):
        source = self.router_fixture()
        for replacement, error in [
            ('.nest("/v2", external::router())', "not a local function"),
            ('.nest("/v2", missing_routes())', "missing function"),
            (".nest(prefix, v2_routes())", "non-literal"),
            ('.nest("/v2", v2_routes().layer(layer))', "unsupported nested"),
            (".merge(v2_routes())", "unsupported router merge"),
            ('.nest_service("/v2", service)', "unsupported router nest_service"),
            (".fallback(handler)", "unsupported router fallback"),
            ('.nest("/v2", router())', "recursive router nesting"),
        ]:
            with (
                self.subTest(replacement=replacement),
                self.assertRaisesRegex(ValueError, error),
            ):
                boundary.inventory(
                    source.replace('.nest("/v2", v2_routes())', replacement), "fixture"
                )

    def test_exports_in_additional_and_nested_modules_are_checked(self):
        for side, marker, name in [
            ("wasm", "#[wasm_bindgen]", "BoltzClient"),
            ("native", "#[uniffi::export]", "SwapClient"),
        ]:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for surface, content in [("wasm", self.wasm), ("native", self.native)]:
                    (root / surface).mkdir()
                    (root / surface / "lib.rs").write_text(content)
                extra = root / side / "nested"
                extra.mkdir()
                (extra / "maker.rs").write_text(
                    f"{marker}\nimpl {name} {{\n    pub async fn broadcast_tx() {{}}\n}}"
                )
                with mock.patch.multiple(
                    boundary, WASM_DIR=root / "wasm", UNIFFI_DIR=root / "native"
                ):
                    wasm, native = boundary.load_binding_sources()
                with self.assertRaisesRegex(
                    ValueError, "raw upstream-only route added"
                ):
                    self.validate(wasm=wasm, native=native)

    def test_missing_proof_gate_has_a_useful_error(self):
        with self.assertRaisesRegex(ValueError, "proof-of-payment gate"):
            self.validate(
                wrappers=self.wrappers.replace(
                    "verify_submarine_preimage", "removed_proof_gate"
                )
            )

    def test_cli_reports_layout_failure_without_traceback(self):
        with tempfile.TemporaryDirectory() as tmp:
            router = Path(tmp) / "router.rs"
            router.write_text("fn router() { Router::new().merge(external()) }")
            result = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts/check_compatibility_boundary.py"),
                    "--maker-router",
                    str(router),
                ],
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("error: unsupported router merge", result.stderr)
            self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
