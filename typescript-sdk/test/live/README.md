# Live suite

Integration tests against a real regtest maker. They move regtest coins and
Lightning payments, so they are opt-in: `npm test` never runs them, and CI
does not either.

```sh
npm run build
KALEIDO_LIVE=1 ... npm run test:live
```

Without `KALEIDO_LIVE=1` every test is skipped.

| File                      | What it proves                                                                                                                                                                                                                                                 |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `submarine.live.test.mjs` | BTC and L-BTC submarine: the maker pays and claims; an underpaid lockup is refunded after the timelock; BTC refunds cooperatively with the create-time and recovered `swapAuth`                                                                                |
| `reverse.live.test.mjs`   | L-BTC reverse: a script-path claim, and the maker's refusal of a cooperative one; a BTC cooperative claim when the maker publishes BTC reverse; the WebSocket reports the swap to `invoice.settled`; `SwapMasterKey.restore` and `restoreIndex` find the swaps |

Each run derives its keys from a fresh random passphrase, so swaps never
collide with an earlier run's, and nothing secret is printed.

## The stack

The suite expects
[kaleidoswap-maker-rs's e2e stack](https://github.com/kaleidoswap/kaleidoswap-maker-rs/tree/main/e2e)
(`e2e/up.sh`): the maker, bitcoind, elementsd, their esploras and the
`ldk-taker` Lightning node with a channel to the maker. The maker must be
0.8.0 or later. 0.7.4 leaves failed payouts at `invoice.pending` and reports
L-BTC reverse lockups before it has broadcast them.

| Variable                      | e2e stack value                                       |
| ----------------------------- | ----------------------------------------------------- |
| `KALEIDO_LIVE`                | `1`                                                   |
| `KALEIDO_LIVE_MAKER_URL`      | `http://127.0.0.1:9420/v2`                            |
| `KALEIDO_LIVE_WS_URL`         | optional; defaults to the maker URL's `/ws`           |
| `KALEIDO_LIVE_BTC_ESPLORA`    | `http://127.0.0.1:3002`                               |
| `KALEIDO_LIVE_LIQUID_ESPLORA` | `http://127.0.0.1:3003`                               |
| `KALEIDO_LIVE_BITCOIND_RPC`   | `http://<user>:<pass>@127.0.0.1:18443/wallet/miner`   |
| `KALEIDO_LIVE_ELEMENTSD_RPC`  | `http://<user>:<pass>@127.0.0.1:18884/wallet/liquidw` |
| `KALEIDO_LIVE_LDK_GRPC`       | `127.0.0.1:13646`                                     |
| `KALEIDO_LIVE_LDK_CERT`       | `<maker-rs>/e2e/run/certs/taker.crt`                  |
| `KALEIDO_LIVE_LDK_API_KEY`    | the taker node's `/data/regtest/api_key`, hex-encoded |

The RPC credentials are in the stack's `docker-compose.regtest.yml`. To read
the API key without printing it:

```sh
export KALEIDO_LIVE_LDK_API_KEY=$(docker exec km-e2e-ldk-taker cat /data/regtest/api_key | od -An -vtx1 | tr -d ' \n')
```

The tests mine blocks on the shared regtest chains, at most one every few
seconds while a swap waits. Faster mining moves the tip under in-flight HTLCs
and the receiving node refuses them as `IncorrectPaymentDetails`.

## What the maker does not do

- **No cooperative spends on Liquid.** The maker co-signs submarine refunds
  and reverse claims for Bitcoin L1 lockups only (`unsupported_cooperative`).
  The tests assert the refusal; L-BTC refunds through the timelock and claims
  by script path.
- **An underpaid submarine is `invoice.failedToPay`**, not
  `transaction.lockupFailed`: the maker records the underpayment, never pays
  out, and the lockup is refundable.
- **No BTC reverse pair is published** on the e2e stack (`BTC/BTC` reverse is
  enabled but not `sdk_visible`), so the BTC cooperative claim skips. To run
  it, publish that pair, for example with `activate_sdk_pair BTC/BTC reverse
BTC BTC` from the stack's `e2e/lib.sh`.
