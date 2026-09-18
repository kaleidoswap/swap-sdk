#!/usr/bin/env node
import { createWriteStream } from "node:fs";
import { access, readFile, rename, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
import { fileURLToPath } from "node:url";

import extract from "extract-zip";

import { ARCHIVES, sha256, validateManifest } from "./native-artifacts.mjs";

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

for (const archive of ARCHIVES) {
  const destination = resolve(packageRoot, archive);
  const partial = `${destination}.part`;
  const url = `https://github.com/kaleidoswap/swap-sdk/releases/download/v${packageJson.version}/${archive}`;
  try {
    const response = await fetch(url, { redirect: "follow" });
    if (!response.ok || !response.body) {
      throw new Error(`download returned HTTP ${response.status}`);
    }
    await finished(Readable.fromWeb(response.body).pipe(createWriteStream(partial)));
    await rename(partial, destination);
    const actual = await sha256(destination);
    const expected = manifest.artifacts[archive].sha256;
    if (actual !== expected) {
      throw new Error(`SHA-256 mismatch for ${archive}: expected ${expected}, got ${actual}`);
    }
    await extract(destination, { dir: packageRoot });
  } catch (error) {
    await rm(partial, { force: true });
    throw new Error(
      `could not install ${url}: ${error.message}. Build from source with ` +
        "'npm run ubrn:build' in a swap-sdk checkout.",
      { cause: error },
    );
  } finally {
    await rm(destination, { force: true });
  }
}
