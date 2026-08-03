REPO_ROOT := $(abspath ..)
CARGO_TARGET_DIR ?= ../target
MATURIN_VERSION ?= 1.14.1
# Pinned by digest: this container compiles the Linux release wheels, so a
# mutable tag would let the build inputs change under a fixed release version.
MANYLINUX_DIGEST ?= sha256:2665227312dd1eab1c29c70a001dc8aac53155a2d048bede3b2df7f1691c8e38
MANYLINUX_IMAGE ?= ghcr.io/pyo3/maturin@$(MANYLINUX_DIGEST)
DOCKER_PLATFORM_ARG := $(if $(MANYLINUX_PLATFORM),--platform $(MANYLINUX_PLATFORM),)
UNAME := $(shell uname)
DYLIB_EXT := $(if $(filter Darwin,$(UNAME)),dylib,so)
DYLIB := libkaleidorg_swap_sdk.$(DYLIB_EXT)
PYTHON_NATIVE_DIR := python/kaleidorg_swap_sdk/kaleidorg_swap_sdk

build-debug:
	cargo build
	mkdir -p $(PYTHON_NATIVE_DIR)
	cp $(CARGO_TARGET_DIR)/debug/$(DYLIB) $(PYTHON_NATIVE_DIR)/
	cargo run --bin uniffi-bindgen -- \
		generate --language python --no-format \
		--library $(CARGO_TARGET_DIR)/debug/$(DYLIB) --out-dir $(PYTHON_NATIVE_DIR)/
	cp scripts/uniffi-package-init.py $(PYTHON_NATIVE_DIR)/__init__.py

generate-python-glue: build-debug
	cp $(PYTHON_NATIVE_DIR)/kaleidorg_swap_sdk.py python/kaleidorg_swap_sdk/_generated_uniffi.py
	cd python && uv run ruff format kaleidorg_swap_sdk/_generated_uniffi.py
	cd python && python3 scripts/prepare_uniffi_fallback.py

build-release:
	cargo build --release
	cargo build  # Also build debug for uniffi to read metadata
	mkdir -p $(PYTHON_NATIVE_DIR)
	cp $(CARGO_TARGET_DIR)/release/$(DYLIB) $(PYTHON_NATIVE_DIR)/
	cargo run --bin uniffi-bindgen -- \
		generate --language python --no-format \
		--library $(CARGO_TARGET_DIR)/debug/$(DYLIB) --out-dir $(PYTHON_NATIVE_DIR)/
	cp scripts/uniffi-package-init.py $(PYTHON_NATIVE_DIR)/__init__.py

build-python:
	cd python && uv run maturin build --release --locked --out dist

build-python-sdist:
	cd python && uv run maturin sdist --out dist

inspect-python-artifacts:
	cd python && python3 scripts/inspect_artifacts.py

inspect-python-wheels:
	cd python && python3 scripts/inspect_artifacts.py --wheel-only

inspect-python-sdist:
	cd python && python3 scripts/inspect_artifacts.py --sdist-only

smoke-python-wheel:
	cd python && uv run python scripts/smoke_artifact.py wheel

smoke-python-sdist:
	cd python && uv run python scripts/smoke_artifact.py sdist

build-python-manylinux:
	docker run --rm $(DOCKER_PLATFORM_ARG) \
		-v "$(REPO_ROOT):/io" \
		-w /io/bindings/python \
		"$(MANYLINUX_IMAGE)" \
		build --release --locked --compatibility manylinux_2_28 --out dist

format-python:
	# Also formats Python tests
	cd python && uv run ruff format . ../tests/bindings

check-python:
	cd python && uv run ruff format --check . ../tests/bindings

test:
	cargo test --package bindings --test test_generated_bindings -- --nocapture
