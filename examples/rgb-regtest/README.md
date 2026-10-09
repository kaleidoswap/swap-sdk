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
git clone https://github.com/kaleidoswap/kaleidoswap-maker-rs.git maker-rgb-regtest
cd maker-rgb-regtest
git checkout --detach 49c6ce2e9554eea0d819fd3bbd267af6db45b1c0
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

## TypeScript bindings: both directions and refunds

The TypeScript driver uses the built Node facade and WebAssembly for **every
swap create, response validation, template freeze and HTLC signature**. Its
private JSON-line Rust child supplies real rgb-lib wallets, Bitcoin Core and
Lightning operations; it does not create swaps or sign HTLCs. The bridge reads
actual rgb-lib allocations after coloring and preserves the wasm HTLC witness
when the wallet signs the refund's additional BTC input.

Build the maker from `feat/528-rgb-onchain-swaps` rather than the historical
native-run revision above. The recorded TypeScript run used `866215f`.
The same pinned LDK image is compatible with that revision. Set
`RGB_MAKER_BIN` to the resulting binary. With a **fresh** owned stack and run
state, build the TypeScript package and the wallet bridge, then run:

```sh
npm --prefix typescript-sdk run build
cargo +1.96.0 build --manifest-path examples/rgb-regtest/Cargo.toml --locked
examples/rgb-regtest/regtest.sh up
examples/rgb-regtest/target/debug/rgb-sdk-regtest bootstrap
RGB_MAKER_REVISION=$(git -C /absolute/path/to/maker rev-parse HEAD) \
RGB_SDK_REVISION=$(git rev-parse HEAD) \
node examples/rgb-regtest/ts-run.ts
```

Node 22.18 or newer can execute the TypeScript source directly. Type checking:

```sh
typescript-sdk/node_modules/.bin/tsc -p examples/rgb-regtest/tsconfig.json
```

The driver checks submarine success, a cancelled-invoice submarine refund,
a reverse claim from a wallet with zero BTC, and an abandoned reverse swap.
The last case keeps the preimage private, mines past the refund deadline, and
observes the maker's automatic colored refund plus failure of the real held
Lightning payment. It audits settled maker inventory after stopping the daemon
and confirms conservation of all issued RGB units. Its sanitized output is
`run/ts-report.json`; the [recorded TypeScript report](ts-validation-report.json)
contains all four outcomes and their transaction ids.

`ts-bridge` refuses a second attempt over existing `ts-started.json`. Inspect
and preserve recovery state after any failure. Do not reset a chain and then
reuse its wallets. The older successful run and this run need separate fresh
stacks and separate archived `run/` directories.

After validation, keep the maker and deterministic price feed available with:

```sh
examples/rgb-regtest/target/debug/rgb-sdk-regtest serve
```

This foreground supervisor shuts down the maker on Ctrl-C or SIGTERM. It
records its PID in private `run/server-process.json`. Stop the supervisor
before `regtest.sh down`. The TypeScript driver itself stops its daemon after
the final wallet audit; Docker services remain available.

## Exact amount reconciliation

The [amount audit report](amount-audit-report.json) independently checks the
recorded TypeScript run in contract base units, satoshis and millisatoshis.
It recovers the historical deterministic-feed rate by matching the persisted
`pairHash`, recomputes quotes with integer arithmetic, reads actual rgb-lib
transition allocations and spendable wallet UTXOs, matches both Lightning
peers by payment hash, and accounts for every lock/spend input, output and
miner fee. This audit does not create or pay additional swaps.

Stop the owned `serve` supervisor before opening the maker wallet for the
first command, then restart it afterward:

```sh
examples/rgb-regtest/target/debug/rgb-sdk-regtest audit-amounts
examples/rgb-regtest/target/debug/rgb-sdk-regtest audit-lightning
python3 examples/rgb-regtest/amount-audit.py
```

`audit-amounts` reads the isolated PostgreSQL amount/fee snapshot through
Docker and captures actual wallet and Lightning records under ignored
`run/`. `audit-lightning` can run with the maker up; it confirms no unresolved
HTLC balances or msat dust loss and accounts for the channel's commitment
fee and its two 330-sat anchor outputs. The Python check reproduces this
recorded four-flow test's pricing configuration and writes only sanitized
results to `amount-audit-report.json`.

All four quoted RGB amounts exactly match the committed transitions. Final
spendable RGB allocations sum to the full issuance. Both successful LN
payments deliver 100,000,000 msat with zero routing fee; both failed payers
release their HTLCs. The inbound failed-invoice records still read Pending
in LDK's payment bookkeeping; the balance audit establishes that no HTLC
funds remain encumbered.

The audit also records a one-unit (0.000001 test USDT) discrepancy between
the reverse fee breakdown's separately rounded components plus payout and
the gross amount rounded to contract precision. The payout itself exactly
matches the quote: it is rounded down, while each displayed fee is rounded
up. Network allowances are estimates, not exact refunds of actual miner
fees. Bitcoin lock and spend miner fees remain spent on the refund paths.

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


### Cooperative submarine refund

The cooperative refund example attaches to an existing `serve` supervisor and
wallets, and exercises the same authenticated submarine `/refund` endpoint.
It requires the companion maker and the rgb-lib `validate_htlc_spend` addition,
pinned by both native components at `dbdf82dac3ffd2a41b1ec3cc9b410c5a4fa56f2f`.
Build the native components from the coordinated branches:

```sh
cargo +1.96.0 build --manifest-path examples/rgb-regtest/Cargo.toml
# In the companion maker checkout:
# cargo +1.96.0 build -p maker-bin --bin kaleidoswap-maker --features ldk-server
make wasm-pack-build
cd typescript-sdk && npm run build && cd ..
RGB_MAKER_BIN=/path/to/kaleidoswap-maker examples/rgb-regtest/target/debug/rgb-sdk-regtest serve
```

With the supervisor running, in another terminal:

```sh
node --experimental-strip-types examples/rgb-regtest/ts-coop-refund.ts
```

The test creates and locks a held-invoice submarine, rejects cooperation while
Lightning is pending, fails the hold, and then signs/mines a key-path refund
before CLTV. It checks authenticating the request, RGB proof and prevout
mutation rejection, stored-response replay, one-use local signing sessions,
confirmed maker status and exact restored contract units. Only a public result
is saved in `coop-refund-validation-report.json`; recovery and diagnostics
remain in ignored `run/`.

Each attempt uses a new `RGB_COOP_TEST_INDEX` (default 10). An interrupted test
must be recovered deliberately. If it stopped before signing/broadcast and
saved the request, `RGB_COOP_RECOVER=1` resumes the existing prepared operation
with a fresh nonce/session, without recoloring. Recovery refuses operations
that are already broadcast/applied; those require reconciliation, never a
blind rerun. Use a fresh index only after the earlier operation is resolved.
