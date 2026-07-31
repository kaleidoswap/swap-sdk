"""KaleidoSwap SDK Python bindings."""

from importlib.metadata import PackageNotFoundError, version as _distribution_version

try:
    __version__ = _distribution_version("kaleidoswap_sdk")
except PackageNotFoundError:  # running from a source tree, not an install
    __version__ = "0+unknown"

try:
    from .kaleidoswap_sdk import Preimage as _preimage_probe  # noqa: F401
except ImportError:
    # Maturin can omit its generated Python glue from manylinux and sdist
    # wheels while still packaging the native library in this subdirectory.
    from ._generated_uniffi import *  # noqa: F403
else:
    from .kaleidoswap_sdk import *  # noqa: F403
