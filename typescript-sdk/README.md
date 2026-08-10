# `@kaleidorg/swap-sdk`

TypeScript and WebAssembly bindings for KaleidoSwap: Boltz-protocol atomic swaps
(submarine, reverse, and chain) between Bitcoin, Lightning, and Liquid.

## Install

```sh
npm install @kaleidorg/swap-sdk
```

## Usage

`await init()` takes no argument and behaves the same in both runtimes: browsers
resolve the packaged WebAssembly binary relative to the module, and Node reads it
from disk via the `"node"` export condition. Await it once before constructing
any client.

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

## Supplying the binary yourself

To serve the WebAssembly binary from your own CDN, or as a bundler asset URL,
pass any `WasmSource` (a `BufferSource`, `URL`, `Request`, `Response`, or URL
string):

```ts
await init(new URL("/assets/bindings_wasm_bg.wasm", location.origin));
```

`wasmUrl` points at the packaged binary if you need to copy or re-host it. Under
Node you can also pass it straight to `init` — a `file:` source is read from
disk, since Node's `fetch` rejects that scheme. For a pre-compiled
`WebAssembly.Module`, use `initWithModule` — it is separate from `init` so that
`WasmSource` stays type-safe (`WebAssembly.Module` is an empty interface in
TypeScript's lib, so a union containing it accepts any value).

One case needs an explicit source: bundling the Node entry into a single file, as
a CLI or a serverless artifact does, moves `import.meta.url` away from the
packaged `vendor/`, so the default lookup has nothing to find. Copy
`bindings_wasm_bg.wasm` next to your output and pass it — that failure surfaces
at runtime rather than at build time, so reach for this before you ship a bundle.

SDK operations also require the web APIs used by the selected client, including
`fetch` and WebSocket. Node 22 and newer provide both.

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
  Chain swaps claim through `constructCooperativeClaim` instead — the cheaper
  MuSig2 keyspend, partial-signed with the swap's **refund** key rather than its
  claim key. `constructClaim` cannot carry the lockup script that path signs
  against, so a chain swap taking the script path must pass
  `cooperative: false`.
- `SwapMasterKey` — BIP85 swap key and preimage derivation.
- `isKaleidoSwapError` — narrow a rejection to its stable `code`.

Boltz request and response payloads are Rust-defined and cross the boundary as
plain objects typed `any`; the rest of the surface is hand-typed. The packaged
`dist/index.d.ts` carries the full signatures and their documentation.

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
npm run smoke:browser-package
```
