# SDK + rgb-lib live swaps on regtest

This native example exercises the SDK against maker `49c6ce2` and rgb-lib
`96f039d`. Bitcoin Core, Esplora, RGB consignments, SQLite wallet state,
PostgreSQL, the maker daemon and both Lightning nodes are real. A local gRPC
price feed supplies BTC/USDT quotes at 100000 plus 0.000001 per tick. This
tiny change avoids the maker's intentional stale-quote cutoff for an unchanged
price. The issued NIA has 6 decimals and the ticker USDT; it is a regtest test
asset.

The crate has its own workspace and lockfile. rgb-lib is not added to the
SDK's dependencies or wasm build. The crate's secp256k1 source patch unifies
the SDK's pinned beta with RGB consensus's registry beta; both link the same
native library. RGB consensus is pinned to the compatible prerelease.

## Run

Requirements: Docker Compose, OpenSSL, Rust 1.96, a native maker binary with
`ldk-server`, and the pinned LDK server Docker image. From a **separate** maker
checkout at `49c6ce2e9554eea0d819fd3bbd267af6db45b1c0`:

```sh
cargo +1.96.0 build --locked -p maker-bin --bin kaleidoswap-maker --features ldk-server
docker build -f e2e/Dockerfile.ldk-server -t km-e2e/ldk-server:pinned .
```

The explicit `+1.96.0` selects the tested native toolchain even when these
commands run from the SDK root, whose toolchain is Rust 1.88.

The LDK Dockerfile pins `86ca54259b35be612e070d1123a0646d0ac0b389`, including
support for the final CLTV delta on hold invoices. Set the maker binary's
**absolute** path, then from the SDK checkout:

```sh
export RGB_MAKER_BIN=/absolute/path/to/maker/target/debug/kaleidoswap-maker
examples/rgb-regtest/regtest.sh up
cargo +1.96.0 run --manifest-path examples/rgb-regtest/Cargo.toml --locked -- bootstrap
cargo +1.96.0 run --manifest-path examples/rgb-regtest/Cargo.toml --locked -- run
```

`bootstrap` issues 2000 test USDT, sends 1000 to the maker, funds the taker's
fee wallet, and opens a real Lightning channel with liquidity in both
directions. `run` starts the price feed and daemon, disables the unrelated seeded routes and
publishes just the two RGB pairs in the isolated database, then performs:

1. **Submarine success:** validate the pinned contract and HTLC before the
   RGB lock; observe a real invoice payment and maker claim.
2. **Submarine refund:** cancel a real held Lightning payment; require a BTC
   fee input for the 1000-sat HTLC; fund and freeze the SDK template, color
   through rgb-lib, sign the refund with the SDK and the fee input with the
   wallet. Bitcoin Core must reject the refund before CLTV with `non-final`.
   Mine to maturity, broadcast, apply the RGB operation and settle its receive.
3. **Reverse claim:** begin with a fresh RGB wallet with zero BTC; validate
   the maker response before paying the real hold invoice; confirm and
   accept the pinned RGB consignment; color and SDK-sign the HTLC-only claim.
   Verify the on-chain preimage, actual RGB receipt and real invoice settlement.
4. Check settled wallet balances and conservation of all issued units.

Wallet calls run in Tokio `block_in_place`, since rgb-lib uses blocking
clients. `wallet.rs` passes **actual** rgb-lib operation allocations back to
the SDK, rather than deriving them from the requested output map. The wallet
must preserve the SDK's final HTLC witness when signing its own fee input.

## State, recovery and cleanup

The isolated Compose project is `rgb-sdk-regtest`; published ports are
localhost-only: Bitcoin 23443, Esplora 23002, proxy 23000, PostgreSQL 25433,
LDK 23636/23646, maker 29420, price feed 29421. Existing maker/dev stacks are
not used. Pair publication changes only this project's fresh database.

`run/` is ignored by Git. It holds mnemonics, API keys, certificates, daemon
logs, RGB wallets, create responses (including swap credentials), refund keys
and reverse preimages. Recovery JSON files are atomically written with mode
0600 **before** funding/paying or attempting a colored spend broadcast. The
operation file includes the signed PSBT, txid and rgb-lib operation id. Keep
the entire RGB wallet state together with these files until recovery is done.

This is a one-shot validation driver, not a resumable production wallet.
It refuses a second bootstrap or swap run over existing identity/recovery
files. After a failure, inspect the private state before resetting the private
regtest stack. `run/report.json` is a sanitized report containing no keys,
preimages, credentials or proofs; only that report is suitable for sharing.

After inspection:

```sh
examples/rgb-regtest/regtest.sh down
```

This removes only the owned Compose project and its Docker volumes. It keeps
`run/` on disk; archive that directory before creating another fresh run.
Do not reuse its wallets after resetting the chain.

The live run is opt-in and is not part of ordinary SDK CI. Formatting and
linting can be checked without starting services:

```sh
cargo +1.96.0 fmt --manifest-path examples/rgb-regtest/Cargo.toml -- --check
cargo +1.96.0 clippy --manifest-path examples/rgb-regtest/Cargo.toml --locked -- -D warnings
```

## Recorded validation

The [sanitized successful-run report](validation-report.json) records a full
run on 2026-10-06. The refund used one wallet fee input; the reverse claim
used one HTLC input, paid 975 sat and left 546 sat. Settled RGB balances
across the three wallets total the full 2000 test USDT issuance. All three
swaps used the actual maker daemon and real Lightning payments.

The LDK image used for this run was
`sha256:01b4ec516ca4505e602a71eef215bddbeeee66e96d6b3021e9d9686a68bc32da`.
Rebuilding from the pinned Dockerfile may produce another image id. Swap ids,
contract id, transaction ids and the small price step vary between runs;
the recorded report is evidence of that run, not an SDK unit-test fixture.
