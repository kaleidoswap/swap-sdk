# `@kaleidorg/swap-sdk` examples

Four runnable programs against the live KaleidoSwap **signet** maker. Two need
nothing but Node; two move money on signet.

```sh
cd typescript-sdk
npm install && npm run build   # builds the dist/ the examples import
cd examples && npm install     # links @kaleidorg/swap-sdk to the build above
node 01-pairs.ts
```

Node 22+ runs these `.ts` files directly — no bundler, no `tsx`, no build step
of their own. In your own application the import is the bare package name, which
is exactly what these files use.

|                     | What it does                                                                      | Needs                                                |
| ------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `01-pairs.ts`       | Prints every route the maker will trade and on what terms.                        | nothing                                              |
| `02-submarine.ts`   | On-chain BTC → a Lightning invoice. Prints what to fund; refunds with `--refund`. | a swap mnemonic, a BOLT11 invoice, signet bitcoin    |
| `03-reverse.ts`     | A Lightning payment → L-BTC on Liquid. Claims and broadcasts by itself.           | a swap mnemonic, a Liquid address, 50k signet sats   |
| `04-corridor.ts`    | Quotes the Arkade Intents corridor and shows a refusal.                           | nothing                                              |
| `06-atomic.ts`      | Quotes, verifies and tracks an atomic BTC <-> USDT-RGB L1 swap; rgb-lib signs.    | a regtest maker with atomic pairs, an rgb-lib wallet |
| `easy/reverse.ts`   | `03-reverse.ts` on the high-level surface: `quote`, `start`, `phases`.            | the same as `03-reverse.ts`                          |
| `easy/submarine.ts` | `02-submarine.ts` on the high-level surface; refunds on its own if it unwinds.    | the same as `02-submarine.ts`, plus a refund address |
| `easy/resume.ts`    | Finishes or refunds whatever the store still holds; run it at startup.            | a swap mnemonic                                      |

Start with `01-pairs.ts`. If it prints a table, the SDK, its WebAssembly binary
and the maker are all reachable, and everything else is configuration.

## The swap mnemonic

Swap keys and preimages derive from a **swap mnemonic** — a BIP85 child of your
wallet mnemonic (index 26589), never the wallet mnemonic itself:

```ts
const master = SwapMasterKey.fromWalletMnemonic(walletMnemonic, "signet");
console.log(master.swapMnemonic()); // persist this, then use fromSwapMnemonic
```

`KALEIDO_SWAP_INDEX` picks the child key for one swap. **Reusing an index reuses
claim and refund material**, so a real application keeps a persisted counter,
not an environment variable. These examples take it from the environment because
a reader can see it that way.

## Configuration

| Variable                    | Default                   | Meaning                                                      |
| --------------------------- | ------------------------- | ------------------------------------------------------------ |
| `KALEIDO_NETWORK`           | `signet`                  | `signet` (the live maker) or `regtest` (this repo's harness) |
| `KALEIDO_MAKER_URL`         | the network's maker       | Override the maker base URL                                  |
| `KALEIDO_SWAP_INDEX`        | `0`                       | BIP85 child index for this swap — never reuse one            |
| `KALEIDO_SWAP_MNEMONIC`     | —                         | The persisted 12-word swap mnemonic                          |
| `KALEIDO_WAIT_TIMEOUT_SECS` | `3600`                    | How long to wait for a status                                |
| `BOLT11_INVOICE`            | —                         | `02` — the invoice the maker should pay                      |
| `LIQUID_CLAIM_ADDRESS`      | —                         | `03` — where the L-BTC payout lands                          |
| `INVOICE_AMOUNT_SATS`       | `50000`                   | `03` — size of the reverse swap                              |
| `LIQUID_ESPLORA_URL`        | Blockstream liquidtestnet | `03` — Liquid chain access                                   |
| `REFUND_ADDRESS`            | —                         | `02 --refund` — where refunded bitcoin lands                 |

Signet settles on **Mutinynet**. Pair it with Mutinynet chain access
(`https://esplora.signet.kaleidoswap.com`), never a testnet3 endpoint: the two
encode addresses identically, so a mismatch raises no error — swaps are simply
created on one chain and funded or watched on another.

## What these examples do not do

- **They own no wallet.** The SDK does not select inputs or hold keys for your
  funds, so `02` prints an address and an amount and stops. Funding it is your
  wallet's job.
- **They are not a key-storage recommendation.** A real integration keeps the
  swap mnemonic in secure storage and persists the swap id, index, accepted pair
  card, create response and current state _before_ funding or paying.
- **They stop at the Arkade leg.** `04` quotes the corridor; funding or claiming
  the Arkade side needs an Ark wallet, which is the `@kaleidorg/swap-sdk/arkade`
  venue's job.

`02` and `03` write the create response to `examples/swaps/<id>.json` before any
money moves. That file carries the `swapAuth` credential — issued once, never
re-issued — and the material a refund needs. It is gitignored; treat it as
secret.

## Atomic swaps and rgb-lib

`06-atomic.ts` leaves the signing to the wallet's rgb-lib. Use an rgb-lib with
kaleidoswap/rgb-lib#18. Earlier versions sign every input the wallet owns, so a
maker could list one of the taker's coins among its own inputs and take its
value.

`AtomicSwaps.request` therefore takes the wallet's coins, from rgb-lib's
`list_unspents`, and refuses a proposal that lists any of them as the maker's.
The example reads them from `unspents.json`. A list without the request's own
inputs is refused, so an empty one can't be passed by mistake.

Quote expectations are in each asset's base units: sats for BTC, and the RGB
contract's smallest unit for USDT-RGB (6 decimals). They are not the pair
card's display units. `maxServiceFee` bounds only the fee the maker declares,
so set `minToAmount` (or `maxFromAmount` for a `to` quote) to bound the price;
the example requires `ATOMIC_MIN_TO_AMOUNT`.

## RGB wallet adapter

[`05-rgb-spend.ts`](05-rgb-spend.ts) exports claim/refund integration helpers for
an application-provided rgb-lib adapter. It is not a live swap CLI. Create with
an explicit contract pin, validate/accept the lock, then pass the reconstructed
script and PSBT parameters to the helper. It funds before coloring, retains the
immutable funded spend, and returns the signed PSBT plus the wallet operation
ID. The application completes wallet signing, broadcast and RGB operation
application. See [USDT-RGB swaps](../README.md#usdt-rgb-swaps).
