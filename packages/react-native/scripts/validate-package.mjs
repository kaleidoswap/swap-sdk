#!/usr/bin/env node
import { access, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { isGuarded } from "./guard-kaleido-maker.mjs";
import { ARCHIVES, sha256, validateManifest } from "./native-artifacts.mjs";

const required = [
  "src/index.tsx",
  "src/generated/kaleidorg_swap_sdk.ts",
  "react-native.config.js",
  "KaleidoswapSwapSdk.podspec",
  "android/CMakeLists.txt",
  "cpp/generated/kaleidorg_swap_sdk.cpp",
];
const missing = [];
for (const path of required) {
  try {
    await access(resolve(path));
  } catch {
    missing.push(path);
  }
}
if (missing.length > 0) {
  throw new Error(
    `refusing to pack an incomplete native module; run npm run ubrn:build first (missing: ${missing.join(", ")})`,
  );
}

// `ubrn:fixup` adds the in-app refusal to `kaleidoMaker`. A module generated
// without it (by running `ubrn` directly, say) would ship the secret-key
// constructor unguarded.
if (!isGuarded(await readFile("src/generated/kaleidorg_swap_sdk.ts", "utf8"))) {
  throw new Error(
    "refusing to pack a module whose kaleidoMaker is unguarded; run npm run ubrn:fixup",
  );
}

const packageJson = JSON.parse(await readFile("package.json", "utf8"));
const manifest = JSON.parse(await readFile("native-artifacts.json", "utf8"));
validateManifest(manifest, packageJson.version);

for (const archive of ARCHIVES) {
  let actual;
  try {
    actual = await sha256(archive);
  } catch {
    throw new Error(
      `refusing to pack without ${archive}; compress the native build and run npm run native:manifest`,
    );
  }
  const expected = manifest.artifacts[archive].sha256;
  if (actual !== expected) {
    throw new Error(
      `refusing to pack because ${archive} does not match native-artifacts.json`,
    );
  }
}
