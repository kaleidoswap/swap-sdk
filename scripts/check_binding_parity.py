#!/usr/bin/env python3
"""Fail when the wasm binding exposes a capability the UniFFI binding does not.

The two bindings wrap the same core crate but are written by hand, so they drift:
the wasm side grew the swap-key manager, `swap/restore`, the magic-routing hint,
and the chain-swap re-quote calls while the UniFFI side stayed where it was. That
drift is invisible until someone builds a mobile SDK and finds half the API
missing, which is the expensive moment to find it.

Direction is deliberate: wasm is the reference surface, and a UniFFI-only export
is fine (it has real objects where JS has hex strings). Naming differs by
convention — JS drops the `get` prefix — so ALIASES records every intentional
rename, and DIVERGENCES records every capability that is intentionally absent,
with the reason. A new entry in either is a decision; growing them silently is
the failure this guard exists to make loud.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WASM = ROOT / "bindings-wasm/src/lib.rs"
UNIFFI_DIR = ROOT / "bindings/src"

# wasm type name -> UniFFI type name, where they differ.
TYPES = {
    "BoltzClient": "BoltzApiClientV2",
    "WasmSwapMasterKey": "SwapMasterKey",
}

# (wasm type, wasm method) -> UniFFI method, where the name differs.
ALIASES = {
    ("BoltzClient", "create_submarine_swap"): "create_swap",
    ("BoltzClient", "for_network"): "default",
    ("BoltzClient", "for_kaleido_maker"): "kaleido_maker",
    ("BoltzClient", "fee_estimation"): "get_fee_estimation",
    ("BoltzClient", "height"): "get_height",
    ("BoltzClient", "submarine_pairs"): "get_submarine_pairs",
    ("BoltzClient", "reverse_pairs"): "get_reverse_pairs",
    ("BoltzClient", "chain_pairs"): "get_chain_pairs",
    ("BoltzClient", "submarine_tx"): "get_submarine_tx",
    ("BoltzClient", "reverse_tx"): "get_reverse_tx",
    ("BoltzClient", "chain_txs"): "get_chain_txs",
    ("BoltzClient", "submarine_preimage"): "get_submarine_preimage",
    ("BoltzClient", "mrh_bip21"): "get_mrh_bip21",
    ("BoltzClient", "swap"): "get_swap",
    ("BoltzClient", "quote"): "get_quote",
    ("BoltzClient", "nodes"): "get_nodes",
    ("SwapScript", "construct_cooperative_claim"): "submarine_cooperative_claim",
}

# Capabilities the UniFFI binding intentionally does not mirror. Each entry is a
# decision with a reason, not a backlog item.
DIVERGENCES = {
    ("BoltzClient", "request_rfq"): (
        "RfqRequest.profile is a serde_json::Value, which UniFFI cannot "
        "represent. Both corridor routes the SDK models are reachable through "
        "the typed quote_lightning_send / quote_lightning_receive helpers; the "
        "raw escape hatch only buys a route this crate does not serve."
    ),
    ("BtcLikeTransaction", "broadcast"): (
        "UniFFI broadcasts through ChainClient.broadcast_tx, which already "
        "holds the configured Esplora/Electrum client. The wasm binding hangs "
        "it off the transaction because it has no equivalent handle."
    ),
}

IMPL_RE = re.compile(r"^impl\s+(\w+)", re.M)
FN_RE = re.compile(r"^\s*pub (?:async )?fn (\w+)", re.M)


def exports(text: str, marker: str) -> dict[str, set[str]]:
    """Map type name -> exported method names, for impl blocks under `marker`.

    Both bindings put the attribute immediately above `impl`, so splitting on
    the marker and reading the first `impl` line of each chunk is enough; the
    chunk ends at the next top-level `impl`, struct, or enum.
    """
    found: dict[str, set[str]] = {}
    for chunk in text.split(marker)[1:]:
        impl = IMPL_RE.search(chunk)
        if impl is None:
            continue
        body = chunk[impl.end() :]
        end = re.search(r"^(?:impl|pub struct|pub enum|#\[)", body, re.M)
        if end is not None:
            body = body[: end.start()]
        found.setdefault(impl.group(1), set()).update(FN_RE.findall(body))
    return found


def validate(wasm_text: str, uniffi_text: str) -> None:
    wasm = exports(wasm_text, "#[wasm_bindgen]")
    uniffi = exports(uniffi_text, "#[uniffi::export")

    missing: list[str] = []
    for wasm_type, methods in wasm.items():
        target_type = TYPES.get(wasm_type, wasm_type)
        available = uniffi.get(target_type, set())
        for method in sorted(methods):
            if (wasm_type, method) in DIVERGENCES:
                continue
            expected = ALIASES.get((wasm_type, method), method)
            if expected in available:
                continue
            missing.append(
                f"{wasm_type}.{method} -> {target_type}.{expected} "
                f"({'no such type in the UniFFI binding' if not available else 'method absent'})"
            )

    stale = [
        f"{t}.{m}"
        for (t, m) in list(ALIASES) + list(DIVERGENCES)
        if m not in wasm.get(t, set())
    ]

    problems = []
    if missing:
        problems.append(
            "the wasm binding exposes capabilities the UniFFI binding does not:\n  "
            + "\n  ".join(missing)
            + "\n\nAdd them to bindings/src/, or record the decision in "
            "DIVERGENCES with a reason."
        )
    if stale:
        problems.append(
            "ALIASES/DIVERGENCES name wasm exports that no longer exist:\n  "
            + "\n  ".join(sorted(stale))
        )
    if problems:
        raise ValueError("\n\n".join(problems))


def main() -> int:
    uniffi_text = "\n".join(
        path.read_text(encoding="utf-8") for path in sorted(UNIFFI_DIR.glob("*.rs"))
    )
    try:
        validate(WASM.read_text(encoding="utf-8"), uniffi_text)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("UniFFI binding covers every wasm-exposed capability")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
