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

/**
 * Whether an install may finish without the native libraries.
 *
 * Off by default, and that default is set by how npm behaves rather than by
 * what is convenient: a dependency's postinstall output is hidden unless the
 * install runs with `--foreground-scripts`, so a warning here would not reach
 * the consumer. Degrading silently trades a loud install failure that names the
 * problem for a linker error in their app build that does not. A build that has
 * its own reason to proceed — an offline mirror that compiles from source, a
 * CI stage that never links the app — opts in, and accepts the warning it will
 * not see.
 */
export function allowsMissingNativeLibraries(env = process.env) {
  const value = env.KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE;
  if (value === undefined) return false;
  return !["", "0", "false", "no"].includes(value.trim().toLowerCase());
}

/**
 * What a consumer sees when the archives could not be fetched: the archive, the
 * URL, why it failed, and every way forward. npm prints this one, because the
 * install fails with it — which is the argument for failing.
 */
export function unreachableArchiveError({ archive, url, version, reason }) {
  return [
    `${archive} could not be downloaded for @kaleidorg/swap-sdk-react-native@${version}`,
    `  from ${url}`,
    `  ${reason}`,
    "",
    "The native libraries ship as GitHub release assets, not inside the npm",
    "package, so this fails when the release is unreachable — a network that",
    "allows the registry but not github.com, or a release whose assets are gone.",
    "",
    "Ways forward:",
    "  - reinstall once the release is reachable;",
    "  - build from source with 'npm run ubrn:build' in a swap-sdk checkout;",
    "  - set KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE=1 to install without them,",
    "    which leaves this package unable to load until they are present.",
  ].join("\n");
}

/** The same story, for an install that asked to continue without them. */
export function unreachableArchiveWarning({ archive, url, version, reason }) {
  return [
    "",
    "  ┌─ @kaleidorg/swap-sdk-react-native ─────────────────────────────────",
    `  │ WARNING: installed WITHOUT its native libraries (${version}),`,
    "  │ because KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE is set.",
    "  │",
    `  │ ${archive} could not be downloaded from`,
    `  │   ${url}`,
    `  │ ${reason}`,
    "  │",
    "  │ An app built against this package now will fail to link, or crash",
    "  │ when the native module loads. Reinstall once the release is",
    "  │ reachable, or build from source with 'npm run ubrn:build'.",
    "  └────────────────────────────────────────────────────────────────────",
    "",
  ].join("\n");
}

/**
 * What each archive must unpack to, from `ubrn.config.yaml`'s target lists.
 * `postinstall` checks the layout after extracting so a drift in the generator
 * fails the install with a filename, not the app build with a linker error;
 * the release pipeline checks the same lists before sealing the archives.
 */
export const ANDROID_ABIS = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"];
export const ANDROID_LIBRARIES = ANDROID_ABIS.map(
  (abi) => `android/src/main/jniLibs/${abi}/libkaleidorg_swap_sdk.so`,
);
export const IOS_FRAMEWORK = "build/KaleidoSwapSdk.xcframework";
export const IOS_LIBRARIES = [
  `${IOS_FRAMEWORK}/Info.plist`,
  `${IOS_FRAMEWORK}/ios-arm64/libkaleidorg_swap_sdk.a`,
  `${IOS_FRAMEWORK}/ios-arm64_x86_64-simulator/libkaleidorg_swap_sdk.a`,
];
export const NATIVE_LIBRARIES = [...ANDROID_LIBRARIES, ...IOS_LIBRARIES];

/** Every compiled library is on disk under `root`, and none of them is empty. */
export async function assertNativeLibraries(root, stat) {
  const missing = [];
  for (const relative of NATIVE_LIBRARIES) {
    try {
      const info = await stat(`${root}/${relative}`);
      if (info.size === 0) missing.push(`${relative} (empty)`);
    } catch {
      missing.push(relative);
    }
  }
  if (missing.length > 0) {
    throw new Error(`native libraries missing after extraction: ${missing.join(", ")}`);
  }
}
