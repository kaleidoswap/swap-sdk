# L-USDT/BTC swap examples

These examples use the public Rust SDK against a Boltz-compatible KaleidoSwap
Maker. They validate the server-provided invoice binding, script, address, and
asset identifiers before asking the user to move funds. Exact amounts are then
enforced at the funding and claim boundaries.

The directions are:

- `lusdt_submarine`: L-USDT on Liquid to a BTC Lightning invoice.
- `lusdt_reverse`: a BTC Lightning payment to L-USDT on Liquid.

Both examples derive their per-swap key from `SwapMasterKey`. Set
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
BOLT11_INVOICE="lnbcrt..." \
cargo run --example lusdt_submarine
```

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
