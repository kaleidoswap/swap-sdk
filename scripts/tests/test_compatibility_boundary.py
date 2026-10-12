from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
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
        self.wasm = (ROOT / "bindings-wasm/src/lib.rs").read_text()
        self.native = "\n".join(
            path.read_text() for path in sorted((ROOT / "bindings/src").glob("*.rs"))
        )

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
        source = """
fn v2_routes() {
    Router::new().route(
        "/chain/{currency}/height", get(chain::height).post(chain::update))
}
fn swap_routes() {
    Router::new().route("/submarine", post(swap::create).merge(get(pairs::submarine)))
}
fn admin_routes() {
    Router::new().route("/liquid/tx/broadcast", post(admin::broadcast))
        .route_layer(from_fn_with_state(state, auth::require_admin_auth))
}
"""
        inventory = boundary.inventory(source, "fixture")
        self.assertIn(["GET", "/v2/chain/{currency}/height"], inventory["routes"])
        self.assertIn(["POST", "/v2/chain/{currency}/height"], inventory["routes"])
        self.assertIn(["GET", "/v2/swap/submarine"], inventory["routes"])
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


if __name__ == "__main__":
    unittest.main()
