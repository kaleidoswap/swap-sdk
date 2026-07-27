UNAME := $(shell uname)
CARGO_TYPIFY_VERSION ?= 0.7.0

# secp256k1-sys needs a wasm-capable clang to cross-compile its C to wasm.
# Prefer the versioned llvm@21 keg, fall back to unversioned llvm.
ifeq ($(UNAME), Darwin)
	LLVM_PREFIX := $(shell brew --prefix llvm@21 2>/dev/null || brew --prefix llvm 2>/dev/null)
	CLANG_PREFIX += AR=$(LLVM_PREFIX)/bin/llvm-ar CC=$(LLVM_PREFIX)/bin/clang
endif

LND_MACAROON_HEX=$(shell xxd -p regtest/boltz/data/lnd1/data/chain/bitcoin/regtest/admin.macaroon | tr -d '\n')
BITCOIND_COOKIE=$(shell cat regtest/boltz/data/bitcoind/regtest/.cookie)
REGTEST_PREFIX = LND_MACAROON_HEX=$(LND_MACAROON_HEX) BITCOIND_COOKIE=$(BITCOIND_COOKIE)

init:
	cargo install wasm-pack --version 0.14.0 --locked

# --- Release versions --------------------------------------------------------
# The Rust crate, Python distribution, and npm package share one public version.
versions:
	@python3 scripts/release_version.py show

validate-versions:
	@python3 scripts/release_version.py validate

validate-release-version:
	@if [ -z "$(TAG)" ]; then \
		echo "Usage: make validate-release-version TAG=vX.Y.Z" >&2; \
		exit 1; \
	fi
	@bash scripts/validate_release_version.sh "$(TAG)"

sync-version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make sync-version VERSION=X.Y.Z" >&2; \
		exit 1; \
	fi
	@python3 scripts/release_version.py sync "$(VERSION)"

# --- Codegen: RGB Lightning Node (RLN) types --------------------------------
# Generates rln-client/src/types.rs from the OpenAPI 3.1 spec using typify
# (types only; the client is hand-written), matching the kaleido-sdk approach.
# Requires: uv and cargo-typify 0.7.0.
generate-rln-types:
	CARGO_TYPIFY_VERSION=$(CARGO_TYPIFY_VERSION) bash scripts/gen-rln-types.sh

# Generates the Python-side RLN artifacts: pydantic models
# (package-local rln_types.py) + the uniffi.toml custom-type mapping.
# Requires: uv, python3.
generate-rln-pydantic:
	bash scripts/gen-rln-pydantic.sh

# Regenerate every RLN codegen artifact (Rust types + Python models + mapping).
generate-rln: generate-rln-types generate-rln-pydantic

# --- wasm / TypeScript binding ----------------------------------------------
# Builds the wasm-bindgen package (bindings-wasm/pkg) for the browser/TS SDK.
# Needs a wasm-capable clang (see CLANG_PREFIX / `brew install llvm@21`).
WASM_PACK_TARGET ?= web
wasm-pack-build:
	$(CLANG_PREFIX) wasm-pack build bindings-wasm --target $(WASM_PACK_TARGET) --out-dir pkg
	# Vendor the wasm output into the TS package so it is self-contained/publishable.
	rm -rf typescript-sdk/vendor && mkdir -p typescript-sdk/vendor
	cp bindings-wasm/pkg/bindings_wasm.js bindings-wasm/pkg/bindings_wasm.d.ts \
	   bindings-wasm/pkg/bindings_wasm_bg.wasm bindings-wasm/pkg/bindings_wasm_bg.wasm.d.ts \
	   typescript-sdk/vendor/

# Regenerate the TypeScript domain types from the RLN OpenAPI spec.
# Uses the openapi-typescript Node API (scripts/generate-types.mjs) to map
# integer fields to `bigint`, matching the wasm boundary's BigInt serialization.
generate-ts-types:
	cd typescript-sdk && npm ci --ignore-scripts --no-audit --no-fund && npm run generate:types

# Rebuild all OpenAPI-derived sources exclusively from the committed spec and
# repository-pinned tool inputs, then fail if the checked-in outputs drift.
check-generated: generate-rln generate-ts-types
	git diff --exit-code -- \
		rln-client/src/types.rs \
		bindings/python/kaleidoswap_sdk/rln_types.py \
		bindings/uniffi.toml \
		typescript-sdk/src/generated/node-types.ts

build: cargo-build cargo-clippy

cargo-build:
	cargo build -p kaleidoswap-sdk --all-targets --all-features

wasm-build:
	cargo build -p kaleidoswap-sdk --target=wasm32-unknown-unknown --all-features

clippy: cargo-clippy wasm-clippy

test: cargo-test wasm-test

regtest-test: cargo-regtest-test wasm-regtest-test

cargo-clippy:
	cargo clippy -p kaleidoswap-sdk --all-targets --all-features -- -D warnings

cargo-test:
	cargo test -p kaleidoswap-sdk --features "esplora, electrum, lnurl, ws"  -- --nocapture

cargo-regtest-test:
	$(REGTEST_PREFIX) cargo test -p kaleidoswap-sdk --features "electrum, regtest, ws" -- --nocapture

wasm-clippy:
	$(CLANG_PREFIX) cargo clippy -p kaleidoswap-sdk --target=wasm32-unknown-unknown --all-features -- -D warnings

BROWSER ?= firefox

wasm-test:
	$(CLANG_PREFIX) wasm-pack test --headless --$(BROWSER)

wasm-test-chrome:
	BROWSER=chrome $(MAKE) wasm-test

wasm-test-safari:
	BROWSER=safari $(MAKE) wasm-test

wasm-regtest-test:
	$(CLANG_PREFIX) $(REGTEST_PREFIX) WASM_BINDGEN_TEST_TIMEOUT=500 wasm-pack test --headless --$(BROWSER) --features regtest,ws -- regtest

wasm-regtest-test-chrome:
	BROWSER=chrome $(MAKE) wasm-regtest-test

wasm-regtest-test-safari:
	BROWSER=safari $(MAKE) wasm-regtest-test
