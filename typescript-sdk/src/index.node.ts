// Node entry point — selected automatically by the `"node"` condition in this
// package's `exports` map. Browsers and bundlers keep resolving `./index.js`,
// which never references `node:` builtins.
//
// wasm-bindgen's browser loader resolves the .wasm relative to `import.meta.url`
// and `fetch`es it. Node cannot do that: its `fetch` refuses `file:` URLs. Rather
// than pushing that difference onto callers — which previously meant every Node
// consumer hand-wiring `init(await readFile(wasmUrl))` — this entry reads the
// packaged binary itself so `await init()` is correct in both runtimes.

import { readFile } from "node:fs/promises";

import { init as initWithSource, wasmUrl, type WasmSource } from "./index.js";

export * from "./index.js";

/**
 * Load and instantiate the wasm module. Call once (await it) before creating any
 * client. Reads the packaged binary from disk when no {@link WasmSource} is given.
 */
export async function init(
  source?: WasmSource | Promise<WasmSource>,
): Promise<void> {
  await initWithSource(source ?? (await readFile(wasmUrl)));
}
