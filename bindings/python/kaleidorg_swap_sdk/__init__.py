"""KaleidoSwap SDK Python bindings."""

from importlib.metadata import PackageNotFoundError, version as _distribution_version

try:
    __version__ = _distribution_version("kaleidorg_swap_sdk")
except PackageNotFoundError:  # running from a source tree, not an install
    __version__ = "0+unknown"

try:
    from .kaleidorg_swap_sdk import Preimage as _preimage_probe  # noqa: F401
except ImportError:
    # Maturin omits its generated Python glue from manylinux wheels while
    # still packaging the native library in this subdirectory. Measured with
    # Maturin 1.14.1: the manylinux_2_28 wheel ships the .so but no
    # <pkg>/<pkg>/<pkg>.py, so this fallback is load-bearing there. The host
    # (macOS/Windows) and sdist-rebuild paths do include Maturin's own glue and
    # never reach this branch — verify before assuming this is dead code.
    from ._generated_uniffi import *  # noqa: F403
else:
    from .kaleidorg_swap_sdk import *  # noqa: F403
