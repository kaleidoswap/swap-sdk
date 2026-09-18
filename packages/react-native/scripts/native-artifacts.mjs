import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";

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
