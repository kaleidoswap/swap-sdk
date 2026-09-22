#!/usr/bin/env node
import { access, readFile, rename, rm, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import extract from "extract-zip";

import {
  ARCHIVES,
  assertNativeLibraries,
  download,
  allowsMissingNativeLibraries,
  sha256,
  unreachableArchiveError,
  unreachableArchiveWarning,
  validateManifest,
} from "./native-artifacts.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");

// A repository checkout produces these files with `npm run ubrn:build`.
try {
  await access(resolve(repositoryRoot, "Cargo.toml"));
  console.error("postinstall: in-repo checkout, skipping prebuilt download");
  process.exit(0);
} catch {}

const packageJson = JSON.parse(await readFile(resolve(packageRoot, "package.json"), "utf8"));
const manifest = JSON.parse(
  await readFile(resolve(packageRoot, "native-artifacts.json"), "utf8"),
);
validateManifest(manifest, packageJson.version);

const archives = ARCHIVES.map((archive) => ({
  archive,
  url: `https://github.com/kaleidoswap/swap-sdk/releases/download/v${packageJson.version}/${archive}`,
  destination: resolve(packageRoot, archive),
}));

// Two phases, not one loop: every archive is downloaded and verified before
// any is extracted, so a bad digest on the second cannot leave the first's
// binaries on disk beside a failed install.
//
// An unreachable archive is the one failure a consumer can do nothing about
// here, so it is the one an install may be told to survive — but only when
// asked, because npm hides this script's output on a successful install and a
// warning nobody sees is worse than a failure that names the problem.
// Everything the package itself controls — the manifest, the digests, the
// extracted layout — is always fatal: an archive that arrives and is wrong is a
// different problem from one that never arrives.
try {
  let unreachable;
  for (const { archive, url, destination } of archives) {
    const partial = `${destination}.part`;
    try {
      await download(url, partial);
      await rename(partial, destination);
    } catch (error) {
      unreachable = { archive, url, reason: error.message, cause: error };
      break;
    }
    const actual = await sha256(destination);
    const expected = manifest.artifacts[archive].sha256;
    if (actual !== expected) {
      throw new Error(`SHA-256 mismatch for ${archive}: expected ${expected}, got ${actual}`);
    }
  }
  if (unreachable) {
    const details = { ...unreachable, version: packageJson.version };
    if (!allowsMissingNativeLibraries(process.env)) {
      throw new Error(unreachableArchiveError(details), { cause: unreachable.cause });
    }
    console.error(unreachableArchiveWarning(details));
  } else {
    for (const { destination } of archives) {
      await extract(destination, { dir: packageRoot });
    }
    await assertNativeLibraries(packageRoot, stat);
  }
} finally {
  await Promise.all(
    archives.flatMap(({ destination }) => [
      rm(destination, { force: true }),
      rm(`${destination}.part`, { force: true }),
    ]),
  );
}
