#!/usr/bin/env python3
"""Install one built Python artifact in isolation and exercise native code."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path


def venv_python(root: Path) -> Path:
    if sys.platform == "win32":
        return root / "Scripts" / "python.exe"
    return root / "bin" / "python"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("wheel", "sdist"))
    parser.add_argument("--dist", type=Path, default=Path("dist"))
    args = parser.parse_args()

    pattern = "*.whl" if args.kind == "wheel" else "*.tar.gz"
    artifacts = sorted(args.dist.resolve().glob(pattern))
    if len(artifacts) != 1:
        raise ValueError(
            f"expected exactly one {args.kind} in {args.dist}, found {len(artifacts)}"
        )

    with tempfile.TemporaryDirectory(prefix="kaleidoswap-python-smoke-") as temp:
        root = Path(temp)
        subprocess.run(
            ["uv", "venv", "--python", sys.executable, root],
            check=True,
            cwd=root,
        )
        python = venv_python(root)
        subprocess.run(
            ["uv", "pip", "install", "--python", python, artifacts[0]],
            check=True,
            cwd=root,
        )
        subprocess.run(
            [
                python,
                "-I",
                "-c",
                (
                    "from kaleidoswap_sdk import Preimage; "
                    "assert len(Preimage().sha256()) == 64"
                ),
            ],
            check=True,
            cwd=root,
        )

    print(f"{args.kind} clean-install smoke test passed: {artifacts[0].name}")


if __name__ == "__main__":
    main()
