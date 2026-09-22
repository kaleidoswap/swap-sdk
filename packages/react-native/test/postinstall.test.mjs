import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { ARCHIVES } from "../scripts/native-artifacts.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const VERSION = "0.0.0-postinstall-test";

/**
 * A package root that looks like an installed copy: the two scripts, a
 * package.json, and a manifest. Deliberately not inside a checkout — postinstall
 * skips the download entirely when it finds a Cargo.toml two levels up.
 */
function installedPackage(digest = "b".repeat(64)) {
  const root = mkdtempSync(join(tmpdir(), "kaleidorg-postinstall-"));
  mkdirSync(join(root, "scripts"));
  for (const script of ["postinstall.mjs", "native-artifacts.mjs"]) {
    cpSync(join(packageRoot, "scripts", script), join(root, "scripts", script));
  }
  // postinstall imports extract-zip at module load. None of these cases reaches
  // extraction — they end before an archive is on disk — so a stub keeps the
  // test self-contained instead of requiring an install in the temp root. The
  // real extraction path is covered by the release pipeline's install smoke.
  const stubModule = join(root, "node_modules", "extract-zip");
  mkdirSync(stubModule, { recursive: true });
  writeFileSync(
    join(stubModule, "package.json"),
    JSON.stringify({ name: "extract-zip", version: "0.0.0", type: "module", main: "index.js" }),
  );
  writeFileSync(
    join(stubModule, "index.js"),
    "export default async function extract() {\n" +
      "  throw new Error('stub extract-zip must not be reached');\n}\n",
  );
  writeFileSync(join(root, "package.json"), JSON.stringify({ version: VERSION }));
  writeFileSync(
    join(root, "native-artifacts.json"),
    JSON.stringify({
      schema: 1,
      version: VERSION,
      artifacts: Object.fromEntries(ARCHIVES.map((name) => [name, { sha256: digest }])),
    }),
  );
  return root;
}

/** Answer every archive request without a network, and without a real release. */
function stubFetch(root, { status, body }) {
  const stub = join(root, "stub-fetch.mjs");
  writeFileSync(
    stub,
    `globalThis.fetch = async () =>\n` +
      `  new Response(${body === null ? "null" : JSON.stringify(body)}, { status: ${status} });\n`,
  );
  return pathToFileURL(stub).href;
}

function runPostinstall(root, stub, env = {}) {
  const result = spawnSync(process.execPath, ["--import", stub, "scripts/postinstall.mjs"], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
  // The warning goes to stderr and the failures go to stderr; read both either
  // way, so a test cannot pass because it looked at the wrong stream.
  return { status: result.status, output: `${result.stdout ?? ""}${result.stderr ?? ""}` };
}

test("an unreachable archive fails the install and names the ways forward", () => {
  const root = installedPackage();
  try {
    // 404: the release is gone, or its assets were never uploaded. npm prints
    // a failing script's output, so this is where the escape hatch is legible.
    const { status, output } = runPostinstall(root, stubFetch(root, { status: 404, body: null }));
    assert.notEqual(status, 0);
    assert.match(output, new RegExp(`${ARCHIVES[0]} could not be downloaded`));
    assert.match(output, /HTTP 404/);
    assert.match(output, /KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE=1/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE lets the install through", () => {
  const root = installedPackage();
  try {
    const { status, output } = runPostinstall(
      root,
      stubFetch(root, { status: 404, body: null }),
      { KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE: "1" },
    );
    assert.equal(status, 0);
    assert.match(output, /WARNING: installed WITHOUT its native libraries/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("opting in does not excuse an archive that arrives corrupted", () => {
  const root = installedPackage();
  try {
    const { status, output } = runPostinstall(
      root,
      stubFetch(root, { status: 200, body: "not-the-archive" }),
      { KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE: "1" },
    );
    assert.notEqual(status, 0);
    assert.match(output, /SHA-256 mismatch/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an archive that arrives with the wrong digest is still fatal", () => {
  const root = installedPackage();
  try {
    // The carve-out is for archives that never arrive. One that arrives and
    // does not match the manifest is tampering or corruption, and degrading
    // there would install whatever a hostile mirror served.
    const { status, output } = runPostinstall(
      root,
      stubFetch(root, { status: 200, body: "not-the-archive" }),
    );
    assert.notEqual(status, 0);
    assert.match(output, /SHA-256 mismatch/);
    assert.doesNotMatch(output, /WARNING: installed WITHOUT/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a manifest that does not match the package is still fatal", () => {
  const root = installedPackage("not-a-digest");
  try {
    const { status, output } = runPostinstall(root, stubFetch(root, { status: 404, body: null }));
    assert.notEqual(status, 0);
    assert.match(output, /no valid SHA-256/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
