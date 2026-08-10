# `@kaleidorg/swap-sdk`

TypeScript and WebAssembly bindings for KaleidoSwap: Boltz-protocol atomic swaps
(submarine, reverse, and chain) between Bitcoin, Lightning, and Liquid.

## Install

```sh
npm install @kaleidorg/swap-sdk
```

## Browser usage

The `0.1.x` package is browser-first and expects WebAssembly, `fetch`, and
WebSocket support. Await `init()` once before constructing any client.

```ts
import { BoltzClient, init } from "@kaleidorg/swap-sdk";

await init();
const boltz = BoltzClient.forNetwork("signet");
const pairs = await boltz.submarinePairs();
```

`"signet"` reaches the live KaleidoSwap maker, so the snippet above runs as
written. `"regtest"` resolves to `http://localhost:9001/v2` and needs this
repository's local harness.

Bundlers must emit the packaged `vendor/bindings_wasm_bg.wasm` asset referenced
by the generated module.

`BoltzClient.forNetwork` resolves the default **KaleidoSwap maker**, which today
serves `"signet"` and `"regtest"` only. `"mainnet"` and `"testnet"` are rejected
rather than silently falling back to a third-party maker — reach one of those by
passing an explicit base URL to `new BoltzClient(baseUrl, timeoutSecs?)`. Signet
settles on Mutinynet, so pair it with Mutinynet chain access
(`https://esplora.signet.kaleidoswap.com`), never a testnet3 endpoint: the two
encode addresses identically, so a mismatch raises no error and simply creates
swaps on one chain while funding or watching another.

## Node usage

Node 22 and newer can import and initialize the package, but must supply the
packaged WASM bytes because Node does not fetch `file:` URLs:

```ts
import { readFile } from "node:fs/promises";
import { init, wasmUrl } from "@kaleidorg/swap-sdk";

await init(await readFile(wasmUrl));
```

SDK operations also require the web APIs used by the selected client, including
`fetch` and WebSocket. Browser behavior is the primary supported runtime for
`0.1.x`.

## Swap keys

Swap keys and preimages derive client-side from a wallet mnemonic via BIP85
index 26589 — no key material leaves the caller:

```ts
import { SwapMasterKey } from "@kaleidorg/swap-sdk";

const master = SwapMasterKey.fromWalletMnemonic(walletMnemonic, "regtest");
const { publicKey, secretKey } = master.deriveSwapKey(0n);
```

## Typed surface

- `BoltzClient` — Boltz swap API (create submarine/reverse/chain swaps, pairs,
  fees, quotes, restore).
- `BoltzWsApi` / `BoltzWsUpdates` — WebSocket swap-status stream.
- `SwapScript` — reconstruct a swap from its creation response, then build
  claim/refund transactions (`constructClaim`, `constructRefund`) or
  caller-funded Liquid PSETs (`prepareLiquidClaim`, `prepareLiquidRefund`).
- `SwapMasterKey` — BIP85 swap key and preimage derivation.
- `isKaleidoSwapError` — narrow a rejection to its stable `code`.

Boltz request and response payloads are Rust-defined and cross the boundary as
plain objects typed `any`; the rest of the surface is hand-typed. See
`src/index.ts` for the full signatures.

## Lossless integer values

Amounts cross the WASM boundary as `bigint`. Use the exported `toJson` helper
when serializing SDK responses:

```ts
import { toJson } from "@kaleidorg/swap-sdk";

console.log(toJson({ amount: 1000n }));
```

## Development checks

Build fresh WASM bindings from the repository root before running the package
checks:

```sh
make wasm-pack-build
cd typescript-sdk
npm ci
npm run typecheck
npm run lint
npm run format:check
npm test
npm run smoke:package
```
