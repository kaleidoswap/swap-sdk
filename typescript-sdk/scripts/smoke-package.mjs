import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const consumerRoot = mkdtempSync(join(tmpdir(), "kaleidoswap-sdk-npm-"));
let tarballPath;

try {
  const packResult = JSON.parse(
    execFileSync("npm", ["pack", "--json"], {
      cwd: packageRoot,
      encoding: "utf8",
    }),
  );
  const [{ filename, files }] = packResult;
  tarballPath = join(packageRoot, filename);

  const paths = files.map(({ path }) => path);
  const requiredPaths = [
    "LICENSE",
    "README.md",
    "dist/index.d.ts",
    "dist/index.js",
    "package.json",
    "vendor/bindings_wasm.d.ts",
    "vendor/bindings_wasm.js",
    "vendor/bindings_wasm_bg.wasm",
    "vendor/bindings_wasm_bg.wasm.d.ts",
  ];
  const missing = requiredPaths.filter((path) => !paths.includes(path));
  if (missing.length > 0) {
    throw new Error(
      `npm package is missing required files: ${missing.join(", ")}`,
    );
  }

  const allowedRoots = ["dist/", "vendor/"];
  const unexpected = paths.filter(
    (path) =>
      !requiredPaths.includes(path) &&
      !allowedRoots.some((root) => path.startsWith(root)),
  );
  if (unexpected.length > 0) {
    throw new Error(
      `npm package contains unexpected files: ${unexpected.join(", ")}`,
    );
  }

  writeFileSync(
    join(consumerRoot, "package.json"),
    JSON.stringify({ private: true, type: "module" }),
  );
  execFileSync("npm", ["install", "--ignore-scripts", tarballPath], {
    cwd: consumerRoot,
    stdio: "inherit",
  });

  writeFileSync(
    join(consumerRoot, "smoke.mjs"),
    `
      import { readFile } from "node:fs/promises";
      import {
        SwapMasterKey,
        init,
        toJson,
        wasmUrl,
      } from "@kaleidoswap/sdk";

      await init(await readFile(wasmUrl));
      const key = SwapMasterKey.fromWalletMnemonic(
        "slogan prevent affair connect autumn crop together earn track ribbon horn copy",
        "regtest",
      );
      if (key.masterXpub().length === 0) {
        throw new Error("native key derivation returned an empty xpub");
      }
      if (toJson({ amount: 1n }) !== '{"amount":"1"}') {
        throw new Error("bigint JSON encoding failed");
      }
    `,
  );
  execFileSync("node", ["smoke.mjs"], {
    cwd: consumerRoot,
    stdio: "inherit",
  });

  const packed = readFileSync(tarballPath);
  if (packed.length === 0) {
    throw new Error("npm pack produced an empty archive");
  }
  console.log("npm package smoke test passed");
} finally {
  if (tarballPath) {
    rmSync(tarballPath, { force: true });
  }
  rmSync(consumerRoot, { recursive: true, force: true });
}
