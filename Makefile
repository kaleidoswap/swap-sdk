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
	cargo build --all-targets --all-features

wasm-build:
	cargo build --target=wasm32-unknown-unknown --all-features

clippy: cargo-clippy wasm-clippy

test: cargo-test wasm-test

regtest-test: cargo-regtest-test wasm-regtest-test

cargo-clippy:
	cargo clippy --all-targets --all-features -- -D warnings

cargo-test:
	cargo test --features "esplora, electrum, lnurl, ws"  -- --nocapture

cargo-regtest-test:
	$(REGTEST_PREFIX) cargo test --features "electrum, regtest, ws" -- --nocapture

wasm-clippy:
	$(CLANG_PREFIX) cargo clippy --target=wasm32-unknown-unknown --all-features -- -D warnings

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
