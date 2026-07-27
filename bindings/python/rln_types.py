"""Source-tree compatibility module for UniFFI's standalone binding tests.

Published wheels expose these models as ``kaleidoswap_sdk.rln_types``. The
legacy UniFFI test harness generates a standalone ``kaleidoswap_sdk.py`` next to
this file, so execute the package-local generated models in this module.
"""

from pathlib import Path

_models = Path(__file__).with_name("kaleidoswap_sdk") / "rln_types.py"
exec(compile(_models.read_bytes(), str(_models), "exec"))
