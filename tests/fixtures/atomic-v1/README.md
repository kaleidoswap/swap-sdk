# Atomic BTC <-> USDT-RGB wire capture, version 1

`wire-capture.json` holds every HTTP body of two real atomic swaps, one per
pair, exactly as they crossed the wire: the taker's quote request, rgb-lib
request and signed completion, and the maker's quote, proposal, broadcast and
final status. Each body is stored as a JSON string so its numbers keep every
digit; the seal blindings are above 2^53. `now` is the taker's clock when the
quote arrived.

The capture is maker-rs's `crates/maker-api/tests/atomic_regtest.rs`, run
unchanged except that it records each body and asks for quotes with
`"direction": "from"`, as the SDK does. Both swaps settled
(`transaction.claimed`) on rgb-lib's regtest esplora and RGB proxy.

Pinned sources, captured 2026-10-13:

- kaleidoswap-maker-rs `3bda0902` (main).
- kaleidoswap/rgb-lib `3b951094285f5fa606a63c1eee6ad167f1ab689f`, the
  maker's pin.

The wallets, the issued asset and the chain were disposable. There are no
mnemonics or keys here, only what the maker and the taker show each other:
PSBTs, outpoints, consignment endpoints and blindings. Swap ids, amounts and
timestamps change on recapture; freeze one capture and never regenerate it in
tests.

## Run the SDK check

```sh
BITCOIND_SKIP_DOWNLOAD=1 ELEMENTSD_SKIP_DOWNLOAD=1 cargo test --test atomic_contract
```

The TypeScript package replays the same file in `test/atomic.test.mjs`.

## Recapture

Start rgb-lib's regtest services (`tests/compose.yaml`, profile `tests`,
services `esplora` and `proxy`). Then make the maker test write each body to
`$ATOMIC_CAPTURE_DIR`: record the sent body and the raw response text around
each POST, and `GET /v2/swap/atomic/{id}` after the swap settles. Run it as
`atomic_regtest`'s module doc describes. Copy the bodies into
`wire-capture.json` and run both SDK checks again.
