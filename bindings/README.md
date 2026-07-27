# Bindings

Uniffi bindings for the KaleidoSwap SDK (the Boltz-protocol swap engine + the RGB Lightning Node client).

[UniFFI](https://mozilla.github.io/uniffi-rs/) enables automatic generation of bindings for multiple programming languages from a single Rust codebase. Currently, only Python bindings are implemented in this repository, but UniFFI supports many other languages including Kotlin, Swift, Ruby, and more.

## Python

### Installation

#### From PyPI

```bash
pip install kaleidoswap_sdk
```

#### From Source

1. **Prerequisites**

   - Rust toolchain (1.70+)
   - Python 3.11+
   - `uv` package manager

2. **Build the bindings**

   ```bash
   cd bindings
   make build-python
   ```

3. **Install the wheel**
   ```bash
   cd python/dist
   pip install kaleidoswap_sdk-*.whl
   ```

## Development

### Building from Source

The Makefile provides several build targets:

```bash
# Build debug version with Python bindings
make build-debug

# Build release version with Python bindings
make build-release

# Build Python package (wheel)
make build-python

# Build release Python artifacts in a manylinux container
make build-python-manylinux
```

`build-python` uses Maturin's UniFFI backend and creates a correctly
platform-tagged wheel. Use `build-python-manylinux` for Linux release artifacts;
the Maturin manylinux image builds and audits the wheel directly. Set
`MANYLINUX_PLATFORM=linux/arm64` to build the aarch64 variant on a host with
container emulation support.

Build the source distribution separately:

```bash
make build-python-sdist
make inspect-python-artifacts
make smoke-python-wheel
make smoke-python-sdist
```

The sdist contains Rust and Python source and must not contain prebuilt `.so`,
`.dylib`, or `.dll` files. `inspect-python-artifacts` enforces the native wheel
tag, required package contents, and the absence of regtest credentials or
prebuilt libraries in the sdist. The smoke targets create isolated environments,
install the selected archive, import the package-local RLN models, and execute a
native SDK operation.

### Generated Files

The build process generates:

- `python/dist/kaleidoswap_sdk-*-<platform>.whl` - native wheel
- `python/dist/kaleidoswap_sdk-*.tar.gz` - source distribution

Maturin generates the UniFFI Python module and packages it with the native
library under the `kaleidoswap_sdk` package. A platform-independent fallback of
that generated Python glue is committed as
`python/kaleidoswap_sdk/_generated_uniffi.py` and checked for drift. The package
imports it only if Maturin omits its own generated module during a manylinux or
sdist build. Native libraries remain uncommitted build artifacts.

### Testing

Tests can be run from the root of the repository with `make test` in the [regtest environment](README.md#testing).
