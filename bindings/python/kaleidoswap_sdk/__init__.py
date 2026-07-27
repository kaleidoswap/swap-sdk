"""KaleidoSwap SDK Python bindings."""

import sys

from . import rln_types

# UniFFI custom-type configuration emits a normal ``import rln_types``. Keep
# that generated detail private while exposing the models from this package.
sys.modules.setdefault("rln_types", rln_types)

try:
    from .kaleidoswap_sdk import Preimage as _preimage_probe  # noqa: F401
except ImportError:
    # Maturin can omit its generated Python glue from manylinux and sdist
    # wheels while still packaging the native library in this subdirectory.
    from ._generated_uniffi import *  # noqa: F403
else:
    from .kaleidoswap_sdk import *  # noqa: F403
