#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { argv } from "node:process";

import { ARCHIVES, sha256, validateManifest } from "./native-artifacts.mjs";

const files = argv.slice(2).map((file) => resolve(file));
if (files.length !== ARCHIVES.length) {
  throw new Error(`usage: create-native-manifest.mjs ${ARCHIVES.join(" ")}`);
}

const version = JSON.parse(await readFile("package.json", "utf8")).version;
const artifacts = Object.fromEntries(
  await Promise.all(
    files.map(async (file) => [basename(file), { sha256: await sha256(file) }]),
  ),
);
const manifest = { schema: 1, version, artifacts };
validateManifest(manifest, version);
await writeFile("native-artifacts.json", `${JSON.stringify(manifest, null, 2)}\n`);
