# `@kaleidoswap/sdk`

TypeScript and WebAssembly bindings for KaleidoSwap atomic swaps and the RGB
Lightning Node client.

## Install

```sh
npm install @kaleidoswap/sdk@0.1.0
```

## Browser usage

The `0.1.x` package is browser-first and expects WebAssembly, `fetch`, and
WebSocket support.

```ts
import { init, RlnClient } from "@kaleidoswap/sdk";

await init();
const client = RlnClient.connect("https://node.example");
```

Bundlers must emit the packaged `vendor/bindings_wasm_bg.wasm` asset referenced
by the generated module.

## Node usage

Node 22 and newer can import and initialize the package, but must supply the
packaged WASM bytes because Node does not fetch `file:` URLs:

```ts
import { readFile } from "node:fs/promises";
import { init, wasmUrl } from "@kaleidoswap/sdk";

await init(await readFile(wasmUrl));
```

SDK operations also require the web APIs used by the selected client, including
`fetch` and WebSocket. Browser behavior is the primary supported runtime for
`0.1.x`.

## Lossless integer values

Amounts cross the WASM boundary as `bigint`. Use the exported `toJson` helper
when serializing SDK responses:

```ts
import { toJson } from "@kaleidoswap/sdk";

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
