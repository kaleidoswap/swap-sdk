"""KaleidoSwap SDK Python bindings."""

import sys
from importlib.metadata import PackageNotFoundError, version as _distribution_version

from . import rln_types

try:
    __version__ = _distribution_version("kaleidoswap_sdk")
except PackageNotFoundError:  # running from a source tree, not an install
    __version__ = "0+unknown"

# UniFFI custom-type configuration emits a normal ``import rln_types``. Keep
# that generated detail private while exposing the models from this package.
#
# This claims a top-level module name, so it can lose to a consumer that already
# owns ``rln_types``. Fail loudly here rather than deep inside an FFI conversion,
# where the symptom is an inscrutable AttributeError on the wrong module.
if sys.modules.setdefault("rln_types", rln_types) is not rln_types:
    raise ImportError(
        "kaleidoswap_sdk needs the top-level module name 'rln_types', but "
        f"{sys.modules['rln_types']!r} already owns it. Import kaleidoswap_sdk "
        "before that module, or rename it."
    )

try:
    from .kaleidoswap_sdk import Preimage as _preimage_probe  # noqa: F401
except ImportError:
    # Maturin can omit its generated Python glue from manylinux and sdist
    # wheels while still packaging the native library in this subdirectory.
    from ._generated_uniffi import *  # noqa: F403
else:
    from .kaleidoswap_sdk import *  # noqa: F403
