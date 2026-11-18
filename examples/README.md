# Rust examples

Two folders:

- **`simple/`** uses the high-level `kaleidoswap::KaleidoSwap` surface: you name
  what you pay and what you receive, and the SDK prices the route, checks the
  maker's response, persists the swap, and claims or refunds on its own. They
  run against the signet maker. Start here.
- **`advanced/`** drives the swap engine step by step: pair cards, create
  requests, response validation, scripts, claim and refund transactions. Use it
  when you own those steps yourself, or for a route the high-level surface does
  not serve yet (L-USDT, the Arkade corridor).

Cargo only discovers top-level examples, so each one is named in `Cargo.toml`
and run by that name.

## `simple/`

| `cargo run --example` | What it does                                                        | Needs                                                      |
| --------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------- |
| `simple_submarine`    | On-chain BTC → a Lightning invoice; refunds on its own if it fails. | `KALEIDO_SWAP_MNEMONIC`, `BOLT11_INVOICE`, `REFUND_ADDRESS` |
| `simple_reverse`      | A Lightning payment → L-BTC on Liquid; claims on its own.           | `KALEIDO_SWAP_MNEMONIC`, `LIQUID_CLAIM_ADDRESS`             |

Their store, which holds every swap until it ends, is `examples/swaps/simple/`.
It is gitignored; treat it as secret. It hands out each swap index exactly once,
so there is no `KALEIDO_SWAP_INDEX` to manage.

## `advanced/`

| `cargo run --example`       | What it does                                                                                 |
| --------------------------- | -------------------------------------------------------------------------------------------- |
| `advanced_submarine`        | On-chain BTC → a Lightning invoice: create, validate, persist, print what to fund, refund if the maker cannot pay. |
| `advanced_reverse`          | A Lightning payment → on-chain BTC: create, validate, check the timeout, persist, claim on confirmation. |
| `lusdt_submarine`           | L-USDT on Liquid → a BTC Lightning invoice.                                                  |
| `lusdt_reverse`             | A BTC Lightning payment → L-USDT on Liquid.                                                  |
| `corridor_quote`            | The Arkade Intents corridor in both directions: quote, pre-commit gates, track. Funds nothing. |
| `signet_smoke`              | Live checks against the signet maker: defaults, pairs, chain tip, a validated unfunded create. |
| `kaleido_attribution_probe` | Checks that a swap created through `KaleidoMakerClient` arrives attributed at the maker.     |
| `multiout_battletest`       | Reverse-swap claims that pay two outputs. Creates real swaps.                                |

They take the network and maker from `KALEIDO_NETWORK` (default `regtest`) and
`KALEIDO_MAKER_URL` (default `http://localhost:9001/v2`), and chain access from
`BITCOIN_ESPLORA_URL` or `LIQUID_ESPLORA_URL`. `advanced_submarine` and
`advanced_reverse` write each create response to
`examples/swaps/advanced/<id>.json` before any money moves: it carries the
`swap_auth` credential, issued once, that a cooperative refund or claim needs.

`corridor_quote` is safe to run as-is against signet: its default inputs are
undecodable on purpose, so the maker refuses before reserving anything.

## L-USDT/BTC swaps

These examples use the public Rust SDK against a Boltz-compatible KaleidoSwap
Maker. They validate the server-provided invoice binding, script, address, and
asset identifiers before asking the user to move funds. Exact amounts are then
enforced at the funding and claim boundaries.

The engine-level examples derive their per-swap key from `SwapMasterKey`. Set
`KALEIDO_SWAP_MNEMONIC` to the persisted 12-word **swap mnemonic**, not a
disposable value. Increment `KALEIDO_SWAP_INDEX` for every new swap. Reusing an
index reuses claim/refund material and is unsafe.

Create the swap mnemonic once from the wallet mnemonic, persist the resulting
`SwapMasterKey` securely, and reuse it:

```rust
use kaleidorg_swap_sdk::network::Network;
use kaleidorg_swap_sdk::util::secrets::SwapMasterKey;

let master = SwapMasterKey::new(wallet_mnemonic, wallet_passphrase, Network::Mainnet)?;
println!("persist this swap mnemonic securely: {}", master.mnemonic);
# Ok::<(), kaleidorg_swap_sdk::error::Error>(())
```

## Submarine: L-USDT to BTC Lightning

```bash
KALEIDO_MAKER_URL=http://localhost:9001/v2 \
KALEIDO_NETWORK=regtest \
KALEIDO_SWAP_MNEMONIC="your persisted swap mnemonic" \
KALEIDO_SWAP_INDEX=0 \
LUSDT_ASSET_ID="<the deployment's L-USDT asset id>" \
BOLT11_INVOICE="lnbcrt..." \
cargo run --example lusdt_submarine
```

`LUSDT_ASSET_ID` pins the asset the swap must carry. It is required on regtest
and testnet, where USDT is issued per deployment; on mainnet it may be omitted
(the canonical Liquid USDt is used) and must equal it when set. The maker's
pair card is only cross-checked against the pin, never used as its source.

After validating the response, the example prints:

- the exact L-USDT asset ID advertised by the accepted pair card;
- the exact L-USDT amount and explicit Liquid lockup address;
- the L-BTC policy asset ID the wallet uses for transaction fees.

Fund that output with exactly the printed L-USDT amount. The wallet constructs
the funding transaction; the SDK intentionally does not own or select wallet
inputs. If the happy path fails, the same derived key and swap index are needed
for the timeout refund through `SwapScript::prepare_liquid_refund`.

## Reverse: BTC Lightning to L-USDT

```bash
KALEIDO_MAKER_URL=http://localhost:9001/v2 \
KALEIDO_NETWORK=regtest \
KALEIDO_SWAP_MNEMONIC="your persisted swap mnemonic" \
KALEIDO_SWAP_INDEX=1 \
INVOICE_AMOUNT_SATS=100000 \
LUSDT_ASSET_ID="<the deployment's L-USDT asset id>" \
LUSDT_CLAIM_ADDRESS="el1..." \
LIQUID_ESPLORA_URL=http://localhost:4003/api \
LUSDT_PSET_TEMPLATE=lusdt-reverse-claim-template.json \
LUSDT_FUNDED_PSET=lusdt-reverse-funded-pset.json \
cargo run --example lusdt_reverse
```

The example validates the reverse response and then prints the Lightning
invoice. Pay it only after validation succeeds. Once the Maker lockup confirms,
the example writes a `LiquidPsetTemplate` to `LUSDT_PSET_TEMPLATE`.

An application-specific Liquid wallet adapter must:

1. Decode the template PSET.
2. Preserve the protected swap input and L-USDT payment output.
3. Add a real L-BTC input to pay the Elements fee.
4. Add optional L-BTC change and one explicit fee output, without exceeding
   `maxFee`.
5. Blind outputs when the destination requires it and sign only wallet-owned
   inputs.
6. Write this `FundedLiquidPset` JSON to `LUSDT_FUNDED_PSET`:

```json
{
  "pset": "cHNldP8...",
  "paymentOutputSecrets": {
    "assetId": "<L-USDT asset id>",
    "value": 10000000,
    "assetBlindingFactor": "<32-byte hex>",
    "valueBlindingFactor": "<32-byte hex>"
  }
}
```

The running example verifies the funded PSET, adds only the protected swap
witness, broadcasts the claim, and waits for `invoice.settled`. It will reject
asset substitution, payout skimming, excess fees, duplicate inputs, changed
prevouts, or confidentiality downgrades.

## Common configuration

| Variable | Default | Meaning |
|---|---:|---|
| `KALEIDO_MAKER_URL` | `http://localhost:9001/v2` | Maker v2 REST base URL |
| `KALEIDO_NETWORK` | `regtest` | `mainnet`, `testnet`, `signet` (the KaleidoSwap maker), or `regtest` |
| `KALEIDO_SWAP_INDEX` | `0` | Unique child index for this swap |
| `KALEIDO_WAIT_TIMEOUT_SECS` | `3600` | Status/file wait timeout |

These are reference programs, not a key-storage recommendation. Production
applications should keep the swap mnemonic in secure storage and persist the
swap ID, index, accepted pair card, response, and current state before funding
or paying.
