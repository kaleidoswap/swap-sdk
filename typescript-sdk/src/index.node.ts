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

/** Whether a source names a local file rather than something `fetch` can load. */
function isFileUrl(source: WasmSource): source is URL | string {
  if (source instanceof URL) return source.protocol === "file:";
  return typeof source === "string" && source.startsWith("file:");
}

/**
 * Load and instantiate the wasm module. Call once (await it) before creating any
 * client. Reads the packaged binary from disk when no {@link WasmSource} is given.
 *
 * A `file:` source — including the exported {@link wasmUrl} — is read from disk
 * too, since Node's `fetch` rejects it. Everything else is forwarded unchanged.
 */
export async function init(
  source?: WasmSource | Promise<WasmSource>,
): Promise<void> {
  const resolved = await source;
  if (resolved === undefined) {
    await initWithSource(await readFile(wasmUrl));
  } else if (isFileUrl(resolved)) {
    await initWithSource(await readFile(new URL(resolved)));
  } else {
    await initWithSource(resolved);
  }
}
