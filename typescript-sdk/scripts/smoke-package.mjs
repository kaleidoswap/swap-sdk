import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const consumerRoot = mkdtempSync(join(tmpdir(), "kaleidorg-swap-sdk-npm-"));
const suppliedTarball = process.argv[2];
let tarballPath = suppliedTarball
  ? isAbsolute(suppliedTarball)
    ? suppliedTarball
    : resolve(process.cwd(), suppliedTarball)
  : undefined;
let removeTarball = false;

const requiredPaths = [
  "LICENSE",
  "README.md",
  "dist/index.d.ts",
  "dist/index.js",
  "dist/index.node.d.ts",
  "dist/index.node.js",
  "dist/arkade/index.d.ts",
  "dist/arkade/index.js",
  "package.json",
  "vendor/bindings_wasm.d.ts",
  "vendor/bindings_wasm.js",
  "vendor/bindings_wasm_bg.wasm",
  "vendor/bindings_wasm_bg.wasm.d.ts",
];
const allowedRoots = ["dist/", "vendor/"];

/** Enforce the tarball contents allowlist. Runs for a packed OR supplied archive. */
function assertPackageContents(paths) {
  const missing = requiredPaths.filter((path) => !paths.includes(path));
  if (missing.length > 0) {
    throw new Error(
      `npm package is missing required files: ${missing.join(", ")}`,
    );
  }

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
}

/** List a tarball's members as package-relative paths, matching `npm pack --json`. */
function listTarball(archive) {
  return execFileSync("tar", ["-tzf", archive], { encoding: "utf8" })
    .split("\n")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0 && !entry.endsWith("/"))
    .map((entry) => entry.replace(/^package\//, ""));
}

try {
  if (tarballPath) {
    // A supplied tarball is the exact byte stream a release would publish, so
    // it must satisfy the same allowlist as one we pack ourselves.
    assertPackageContents(listTarball(tarballPath));
  } else {
    const packResult = JSON.parse(
      execFileSync("npm", ["pack", "--json"], {
        cwd: packageRoot,
        encoding: "utf8",
      }),
    );
    const [{ filename, files }] = packResult;
    tarballPath = join(packageRoot, filename);
    removeTarball = true;
    assertPackageContents(files.map(({ path }) => path));
  }

  writeFileSync(
    join(consumerRoot, "package.json"),
    JSON.stringify({ private: true, type: "module" }),
  );
  execFileSync("npm", ["install", "--ignore-scripts", tarballPath], {
    cwd: consumerRoot,
    stdio: "inherit",
  });

  // The browser entry must stay free of `node:` builtins so bundlers need no
  // configuration. Nothing else enforces that: `@types/node` is a dev dependency
  // for the node entry's sake, which makes a stray `node:fs` import in the shared
  // source typecheck cleanly, and the browser smoke test only runs on release.
  const browserEntry = readFileSync(
    join(consumerRoot, "node_modules/@kaleidorg/swap-sdk/dist/index.js"),
    "utf8",
  );
  const nodeSpecifier = browserEntry.match(/["']node:[\w/.-]+["']/);
  if (nodeSpecifier) {
    throw new Error(
      `browser entry references a node: builtin (${nodeSpecifier[0]})`,
    );
  }

  // `main`/`types` are read only by resolvers that skip `exports` — legacy
  // bundlers, `moduleResolution: node10`, some test runners — and those are all
  // Node-ish, so they must land on the node entry. Browsers never consult `main`;
  // the pre-`exports` bundlers that do read the `browser` field instead. Nothing
  // else in CI resolves through either field.
  const manifest = JSON.parse(
    readFileSync(
      join(consumerRoot, "node_modules/@kaleidorg/swap-sdk/package.json"),
      "utf8",
    ),
  );
  for (const [field, expected] of [
    ["main", "dist/index.node.js"],
    ["types", "dist/index.node.d.ts"],
    ["browser", "./dist/index.js"],
  ]) {
    if (manifest[field] !== expected) {
      throw new Error(
        `package.json "${field}" is ${JSON.stringify(manifest[field])}, expected ${JSON.stringify(expected)}`,
      );
    }
  }

  // The `exports` map is the only reason `await init()` works in Node, and its
  // condition order is load-bearing. `--conditions=browser` *adds* to Node's own
  // conditions, so it stands in for an isomorphic bundler that sets both: with
  // "browser" ordered after "node" that case resolves to the Node entry and drags
  // `node:fs/promises` into a browser bundle.
  const entryFor = (conditions) =>
    execFileSync(
      "node",
      [
        ...conditions.map((condition) => `--conditions=${condition}`),
        "--input-type=module",
        "-e",
        'process.stdout.write(import.meta.resolve("@kaleidorg/swap-sdk"))',
      ],
      { cwd: consumerRoot, encoding: "utf8" },
    );
  for (const [conditions, expected] of [
    [[], "dist/index.node.js"],
    [["browser"], "dist/index.js"],
  ]) {
    const resolved = entryFor(conditions);
    if (!resolved.endsWith(expected)) {
      throw new Error(
        `conditions [${conditions.join(", ")}] resolved to ${resolved}, expected ${expected}`,
      );
    }
  }

  // The ./arkade subpath must resolve from the packed tarball's exports map.
  // Resolution only — executing it would need the optional @arkade-os/* peer
  // deps, which a Boltz-only consumer (like this smoke consumer) never
  // installs; that opt-in is the subpath's whole point.
  {
    const resolved = execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        'process.stdout.write(import.meta.resolve("@kaleidorg/swap-sdk/arkade"))',
      ],
      { cwd: consumerRoot, encoding: "utf8" },
    );
    if (!resolved.endsWith("dist/arkade/index.js")) {
      throw new Error(
        `./arkade resolved to ${resolved}, expected dist/arkade/index.js`,
      );
    }
  }

  writeFileSync(
    join(consumerRoot, "smoke.mjs"),
    `
      import { SwapMasterKey, init, toJson } from "@kaleidorg/swap-sdk";

      // Zero-argument init is the call a consumer actually writes, and the only
      // one that behaves identically in Node and the browser. Asserting the
      // hand-wired \`init(await readFile(wasmUrl))\` form instead is what let the
      // missing "./vendor/*" subpath export ship green.
      await init();

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

  // Every way a caller can supply the binary itself, each in its own process:
  // wasm-bindgen memoizes the instantiated module, so a second init() in one
  // process returns early and asserts nothing.
  writeFileSync(
    join(consumerRoot, "smoke-source.mjs"),
    `
      import { readFile } from "node:fs/promises";
      import {
        SwapMasterKey,
        init,
        initWithModule,
        wasmUrl,
      } from "@kaleidorg/swap-sdk";

      const [mode] = process.argv.slice(2);
      if (mode === "bytes") {
        const bytes = await readFile(wasmUrl);
        if (!bytes.byteLength) {
          throw new Error("packaged wasm binary is empty");
        }
        await init(bytes);
      } else if (mode === "file-url") {
        // Node's fetch refuses file: URLs, so the node entry reads them itself.
        await init(wasmUrl);
      } else if (mode === "module") {
        await initWithModule(await WebAssembly.compile(await readFile(wasmUrl)));
      } else {
        throw new Error(\`unknown wasm source mode: \${mode}\`);
      }

      const key = SwapMasterKey.fromWalletMnemonic(
        "slogan prevent affair connect autumn crop together earn track ribbon horn copy",
        "regtest",
      );
      if (key.masterXpub().length === 0) {
        throw new Error(\`\${mode}: native key derivation returned an empty xpub\`);
      }
    `,
  );
  for (const mode of ["bytes", "file-url", "module"]) {
    execFileSync("node", ["smoke-source.mjs", mode], {
      cwd: consumerRoot,
      stdio: "inherit",
    });
  }

  const packed = readFileSync(tarballPath);
  if (packed.length === 0) {
    throw new Error("npm package archive is empty");
  }
  console.log(`npm package smoke test passed: ${tarballPath}`);
} finally {
  if (removeTarball && tarballPath) {
    rmSync(tarballPath, { force: true });
  }
  rmSync(consumerRoot, { recursive: true, force: true });
}
