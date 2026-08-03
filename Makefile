UNAME := $(shell uname)

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

validate-release-readiness: validate-release-version
	@python3 scripts/release_notes.py "$(patsubst v%,%,$(TAG))" >/dev/null
	@echo "Validated local release readiness for $(TAG)"

check-release-workflow:
	@python3 scripts/check_release_workflow.py

test-release-automation:
	@python3 -m unittest discover -s scripts/tests -p 'test_*.py'

sync-version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make sync-version VERSION=X.Y.Z" >&2; \
		exit 1; \
	fi
	@python3 scripts/release_version.py sync "$(VERSION)"

# --- Generated sources -------------------------------------------------------
# Generate the committed platform-independent UniFFI Python glue fallback.
# Maturin normally creates the same files while building a wheel. The package
# imports this snapshot only when cross-platform staging omits generated glue.
generate-python-bindings:
	$(MAKE) -C bindings generate-python-glue

# Regenerate the committed generated sources from repository-pinned inputs,
# then fail if the checked-in outputs drift.
check-generated: generate-python-bindings
	git diff --exit-code -- bindings/python/kaleidorg_swap_sdk/_generated_uniffi.py

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

build: cargo-build cargo-clippy

cargo-build:
	cargo build -p kaleidorg-swap-sdk --all-targets --all-features

wasm-build:
	cargo build -p kaleidorg-swap-sdk --target=wasm32-unknown-unknown --all-features

clippy: cargo-clippy wasm-clippy

test: cargo-test wasm-test

regtest-test: cargo-regtest-test wasm-regtest-test

# Lint every native workspace member. bindings-wasm is excluded because its
# secp256k1 C dependency only cross-compiles under the wasm clang; it is
# covered by wasm-clippy instead.
NATIVE_CRATES = -p kaleidorg-swap-sdk -p bindings -p kaleidorg-swap-sdk-macros

cargo-clippy:
	cargo clippy $(NATIVE_CRATES) --all-targets --all-features -- -D warnings

cargo-test:
	cargo test -p kaleidorg-swap-sdk --features "esplora, electrum, lnurl, ws"  -- --nocapture

cargo-regtest-test:
	$(REGTEST_PREFIX) cargo test -p kaleidorg-swap-sdk --features "electrum, regtest, ws" -- --nocapture

wasm-clippy:
	$(CLANG_PREFIX) cargo clippy -p kaleidorg-swap-sdk -p bindings-wasm --target=wasm32-unknown-unknown --all-features -- -D warnings

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
