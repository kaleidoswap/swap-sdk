#!/usr/bin/env node
/**
 * Clean-consumer smoke test for a packed React Native tarball.
 *
 * The counterpart of typescript-sdk/scripts/smoke-package.mjs for the other
 * npm package: the tarball's inventory must match the allowlist, it must carry
 * no compiled code, its embedded native-artifacts manifest must be valid for
 * its own version, it must install into an empty project, and its `exports`
 * map must resolve to files that are in it.
 *
 * The native binaries are deliberately not exercised here. They arrive via
 * `postinstall` from the GitHub release, which does not exist yet when this
 * runs in the release pipeline, so the install below disables scripts.
 * smoke-react-native-install.mjs is the test that runs postinstall for real,
 * after the release is published.
 */
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";

import { validateManifest } from "./native-artifacts.mjs";

const PACKAGE = "@kaleidorg/swap-sdk-react-native";

// Mirrors NPM_REQUIRED / NPM_ALLOWED_PREFIXES / NPM_FORBIDDEN in
// scripts/react_native_release.py, which enforces the same inventory when the
// release bundle is sealed.
const requiredPaths = [
  "LICENSE",
  "README.md",
  "package.json",
  "native-artifacts.json",
  "react-native.config.js",
  "KaleidoswapSwapSdk.podspec",
  "android/CMakeLists.txt",
  "cpp/generated/kaleidorg_swap_sdk.cpp",
  "src/index.tsx",
  "src/arkade.ts",
  "lib/module/index.js",
  "lib/module/arkade.js",
  "lib/commonjs/index.js",
  "lib/commonjs/arkade.js",
  "lib/typescript/module/index.d.ts",
  "lib/typescript/commonjs/index.d.ts",
  "scripts/postinstall.mjs",
  "scripts/native-artifacts.mjs",
];
const allowedRoots = ["android/", "cpp/", "ios/", "lib/", "src/"];
const forbidden = /\/jniLibs\/|^build\/|\.xcframework(\/|$)|\.(so|a|zip)$/;

const supplied = process.argv[2];
if (!supplied) {
  console.error("usage: smoke-react-native-package.mjs <tarball>");
  process.exit(1);
}
const tarballPath = isAbsolute(supplied) ? supplied : resolve(process.cwd(), supplied);

/** List a tarball's members as package-relative paths. */
function listTarball(archive) {
  return execFileSync("tar", ["-tzf", archive], { encoding: "utf8" })
    .split("\n")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0 && !entry.endsWith("/"))
    .map((entry) => entry.replace(/^package\//, ""));
}

/** Read one member of the tarball without unpacking it. */
function readMember(archive, member) {
  return execFileSync("tar", ["-xzOf", archive, `package/${member}`], { encoding: "utf8" });
}

function assertPackageContents(paths) {
  const missing = requiredPaths.filter((path) => !paths.includes(path));
  if (missing.length > 0) {
    throw new Error(`npm package is missing required files: ${missing.join(", ")}`);
  }
  const unexpected = paths.filter(
    (path) => !requiredPaths.includes(path) && !allowedRoots.some((root) => path.startsWith(root)),
  );
  if (unexpected.length > 0) {
    throw new Error(`npm package contains unexpected files: ${unexpected.join(", ")}`);
  }
  const compiled = paths.filter((path) => forbidden.test(path));
  if (compiled.length > 0) {
    throw new Error(
      `npm package contains compiled code or archives, which must come from the ` +
        `release assets instead: ${compiled.join(", ")}`,
    );
  }
}

const consumerRoot = mkdtempSync(join(tmpdir(), "kaleidorg-swap-sdk-react-native-npm-"));
try {
  const paths = listTarball(tarballPath);
  assertPackageContents(paths);

  const manifest = JSON.parse(readMember(tarballPath, "package.json"));
  if (manifest.name !== PACKAGE) {
    throw new Error(`package.json name is ${JSON.stringify(manifest.name)}, expected ${PACKAGE}`);
  }
  if (manifest.scripts?.postinstall !== "node scripts/postinstall.mjs") {
    throw new Error("package.json does not run scripts/postinstall.mjs on install");
  }
  if (manifest.publishConfig?.provenance !== true) {
    throw new Error("package.json does not set publishConfig.provenance");
  }
  for (const [field, expected] of [
    ["main", "./lib/commonjs/index.js"],
    ["module", "./lib/module/index.js"],
    ["types", "./lib/typescript/commonjs/index.d.ts"],
    ["react-native", "./src/index.tsx"],
  ]) {
    if (manifest[field] !== expected) {
      throw new Error(
        `package.json "${field}" is ${JSON.stringify(manifest[field])}, expected ${JSON.stringify(expected)}`,
      );
    }
  }

  // The manifest inside the tarball is what postinstall will trust. It must be
  // valid for this exact version before anything is published under it.
  validateManifest(JSON.parse(readMember(tarballPath, "native-artifacts.json")), manifest.version);

  writeFileSync(
    join(consumerRoot, "package.json"),
    JSON.stringify({ private: true, type: "module" }),
  );
  // --legacy-peer-deps: `react` and `react-native` are peers of every RN
  // library, and pulling a whole React Native into a smoke consumer proves
  // nothing about this package.
  execFileSync(
    "npm",
    ["install", "--ignore-scripts", "--legacy-peer-deps", "--no-audit", "--no-fund", tarballPath],
    { cwd: consumerRoot, stdio: "inherit" },
  );

  const installed = join(consumerRoot, "node_modules", PACKAGE);
  const resolveImport = (specifier) =>
    execFileSync(
      process.execPath,
      ["--input-type=module", "-e", `process.stdout.write(import.meta.resolve(${JSON.stringify(specifier)}))`],
      { cwd: consumerRoot, encoding: "utf8" },
    );
  const require = createRequire(join(consumerRoot, "noop.js"));
  for (const [label, resolved, expected] of [
    ["import .", resolveImport(PACKAGE), "lib/module/index.js"],
    ["import ./arkade", resolveImport(`${PACKAGE}/arkade`), "lib/module/arkade.js"],
    ["require .", require.resolve(PACKAGE), "lib/commonjs/index.js"],
    ["require ./arkade", require.resolve(`${PACKAGE}/arkade`), "lib/commonjs/arkade.js"],
  ]) {
    if (!resolved.endsWith(expected)) {
      throw new Error(`${label} resolved to ${resolved}, expected ${expected}`);
    }
  }
  for (const relative of ["native-artifacts.json", "scripts/postinstall.mjs", "KaleidoswapSwapSdk.podspec"]) {
    if (!existsSync(join(installed, relative))) {
      throw new Error(`installed package is missing ${relative}`);
    }
  }
  if (existsSync(join(installed, "android/src/main/jniLibs")) || existsSync(join(installed, "build"))) {
    throw new Error("installed package carries native binaries although scripts were disabled");
  }

  if (readFileSync(tarballPath).length === 0) {
    throw new Error("npm package archive is empty");
  }
  console.log(`React Native npm package smoke test passed: ${tarballPath}`);
} finally {
  rmSync(consumerRoot, { recursive: true, force: true });
}
