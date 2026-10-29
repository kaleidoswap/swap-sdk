// Node entry point — selected automatically by the `"node"` condition in this
// package's `exports` map. Browsers and bundlers keep resolving `./index.js`,
// which never references `node:` builtins.
//
// wasm-bindgen's browser loader resolves the .wasm relative to `import.meta.url`
// and `fetch`es it. Node cannot do that: its `fetch` refuses `file:` URLs. Rather
// than pushing that difference onto callers — which previously meant every Node
// consumer hand-wiring `init(await readFile(wasmUrl))` — this entry reads the
// packaged binary itself so `await init()` is correct in both runtimes.

import { mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises";

import {
  init as initWithSource,
  makeCreateKaleidoSwap,
  SwapClient,
  SwapError,
  SwapScript,
  toJson,
  wasmUrl,
  type SwapRecord,
  type SwapStore,
  type WasmSource,
} from "./index.js";

export * from "./index.js";

/** The high-level entry, bound to this entry's `init` so the wasm is read from disk. */
export const createKaleidoSwap = makeCreateKaleidoSwap({
  init: () => init(),
  SwapClient,
  SwapScript,
});

/**
 * A {@link SwapStore} over a directory: one JSON file per swap plus `index`
 * for the BIP85 counter. Writes go through a rename so a crash mid-write
 * leaves the previous file, never a torn one. Single-process: two instances
 * on one directory would hand out the same index.
 */
export class FileStore implements SwapStore {
  private readonly dir: URL;
  private indexLock: Promise<unknown> = Promise.resolve();

  constructor(dir: URL | string) {
    const url =
      typeof dir === "string" ? new URL(dir, `file://${process.cwd()}/`) : dir;
    this.dir = new URL(url.href.endsWith("/") ? url.href : `${url.href}/`);
  }

  /**
   * Ids come from the maker's response, so a name is checked before it
   * touches the filesystem: a swap id can never name a path outside the
   * directory.
   */
  private file(name: string): URL {
    if (!/^[A-Za-z0-9_.-]{1,160}$/.test(name) || name.startsWith("."))
      throw new SwapError(
        "SWAP_FAILED",
        `refusing to use ${name} as a file name`,
      );
    return new URL(name, this.dir);
  }

  private async write(name: string, body: string): Promise<void> {
    // Records carry swapAuth: owner-only, directory and files alike.
    await mkdir(this.dir, { recursive: true, mode: 0o700 });
    const tmp = this.file(`${name}.${process.pid}.${Date.now()}.tmp`);
    await writeFile(tmp, body, { mode: 0o600 });
    await rename(tmp, this.file(name));
  }

  async get(id: string): Promise<SwapRecord | undefined> {
    try {
      return JSON.parse(
        await readFile(this.file(`${id}.json`), "utf8"),
      ) as SwapRecord;
    } catch (error) {
      if ((error as { code?: string }).code === "ENOENT") return undefined;
      throw error;
    }
  }

  async put(record: SwapRecord): Promise<void> {
    await this.write(`${record.id}.json`, toJson(record, 2));
  }

  async list(): Promise<SwapRecord[]> {
    let names: string[];
    try {
      names = await readdir(this.dir);
    } catch (error) {
      if ((error as { code?: string }).code === "ENOENT") return [];
      throw error;
    }
    const records: SwapRecord[] = [];
    for (const name of names) {
      if (!name.endsWith(".json")) continue;
      records.push(
        JSON.parse(await readFile(this.file(name), "utf8")) as SwapRecord,
      );
    }
    return records;
  }

  async nextIndex(): Promise<bigint> {
    const next = this.indexLock.then(async () => {
      let current = 0n;
      try {
        current = BigInt((await readFile(this.file("index"), "utf8")).trim());
      } catch (error) {
        if ((error as { code?: string }).code !== "ENOENT") throw error;
      }
      // The counter advances on disk before the index is handed out, so a
      // crash between the two skips an index rather than reusing one.
      await this.write("index", (current + 1n).toString());
      return current;
    });
    this.indexLock = next.catch(() => {});
    return next;
  }
}

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
