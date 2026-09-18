import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { rm } from "node:fs/promises";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
import { setTimeout as sleep } from "node:timers/promises";

export const ARCHIVES = ["android-artifacts.zip", "ios-artifacts.zip"];

export async function sha256(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

export function validateManifest(manifest, version) {
  if (manifest?.schema !== 1 || manifest?.version !== version) {
    throw new Error("native artifact manifest identity mismatch");
  }
  for (const archive of ARCHIVES) {
    const digest = manifest.artifacts?.[archive]?.sha256;
    if (!/^[0-9a-f]{64}$/.test(digest ?? "")) {
      throw new Error(`native artifact manifest has no valid SHA-256 for ${archive}`);
    }
  }
  const unexpected = Object.keys(manifest.artifacts ?? {}).filter(
    (name) => !ARCHIVES.includes(name),
  );
  if (unexpected.length > 0) {
    throw new Error(`native artifact manifest contains unexpected files: ${unexpected.join(", ")}`);
  }
}

/** A response that a later attempt may get right: throttling or a server error. */
export function isTransientStatus(status) {
  return status === 408 || status === 429 || status >= 500;
}

/**
 * Download `url` to `destination`, bounding every attempt and retrying the
 * failures that a retry can fix.
 *
 * This runs inside `npm install`, where a hung socket is a hung install with
 * nothing on the terminal, so each attempt gets a deadline. A 404 is the
 * release missing its assets and is returned on the first try; a reset or a
 * 5xx from the CDN is retried. Anything left at `destination` on failure is
 * removed so a truncated file can never be mistaken for the archive.
 */
export async function download(
  url,
  destination,
  { fetch = globalThis.fetch, attempts = 3, timeoutMs = 60_000, backoffMs = 2_000 } = {},
) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, {
        redirect: "follow",
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (!response.ok) {
        const error = new Error(`download returned HTTP ${response.status}`);
        error.transient = isTransientStatus(response.status);
        throw error;
      }
      if (!response.body) {
        throw new Error("download returned no body");
      }
      await finished(Readable.fromWeb(response.body).pipe(createWriteStream(destination)));
      return;
    } catch (error) {
      lastError = error;
      await rm(destination, { force: true });
      // A response that says the asset is not there will say so again.
      if (error.transient === false) break;
      if (attempt < attempts) await sleep(backoffMs * attempt);
    }
  }
  throw lastError;
}
