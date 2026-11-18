#!/usr/bin/env node
/**
 * Install the published React Native package from the registry with lifecycle
 * scripts enabled, the way a partner's `npm install` will.
 *
 * This is the one place the package's `postinstall` runs against a real
 * release: the registry serves the sealed tarball, postinstall fetches the
 * archives from the GitHub release just created, and the digests inside the
 * tarball decide whether they are accepted. It ends by checking that every
 * compiled library is on disk — the outcome every earlier byte-for-byte check
 * exists to guarantee, observed directly.
 *
 *   smoke-react-native-install.mjs --version X.Y.Z [--registry URL]
 *                                  [--attempts N] [--delay SECONDS]
 *
 * Registry propagation can lag a publish by a minute, so an install that fails
 * because the version is not visible yet is retried; any other failure — a
 * postinstall digest mismatch above all — is final on the first attempt.
 */
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { setTimeout as sleep } from "node:timers/promises";

import { assertNativeLibraries } from "./native-artifacts.mjs";

const PACKAGE = "@kaleidorg/swap-sdk-react-native";

const { values } = parseArgs({
  options: {
    version: { type: "string" },
    registry: { type: "string", default: "https://registry.npmjs.org" },
    attempts: { type: "string", default: "12" },
    delay: { type: "string", default: "10" },
  },
});
if (!values.version) {
  console.error("usage: smoke-react-native-install.mjs --version X.Y.Z");
  process.exit(1);
}
const attempts = Number(values.attempts);
const delaySeconds = Number(values.delay);

const NOT_VISIBLE_YET = /E404|ETARGET|No matching version|notarget/;

const consumerRoot = mkdtempSync(join(tmpdir(), "kaleidorg-swap-sdk-react-native-install-"));
try {
  writeFileSync(join(consumerRoot, "package.json"), JSON.stringify({ private: true }));
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      // Scripts ON — that is the point. --legacy-peer-deps keeps React Native
      // itself out of a consumer that only needs to prove this package installs.
      execFileSync(
        "npm",
        [
          "install",
          `${PACKAGE}@${values.version}`,
          "--registry",
          values.registry,
          "--legacy-peer-deps",
          "--no-audit",
          "--no-fund",
        ],
        { cwd: consumerRoot, stdio: ["ignore", "inherit", "pipe"], encoding: "utf8" },
      );
      break;
    } catch (error) {
      const stderr = String(error.stderr ?? "");
      process.stderr.write(stderr);
      if (!NOT_VISIBLE_YET.test(stderr) || attempt === attempts) {
        throw new Error(
          `npm install ${PACKAGE}@${values.version} failed on attempt ${attempt}/${attempts}`,
          { cause: error },
        );
      }
      console.log(`registry has not served ${values.version} yet; retrying in ${delaySeconds}s`);
      await sleep(delaySeconds * 1000);
    }
  }

  const installed = join(consumerRoot, "node_modules", PACKAGE);
  await assertNativeLibraries(installed, stat);
  const leftovers = readdirSync(installed).filter((name) => /\.(zip|part)$/.test(name));
  if (leftovers.length > 0) {
    throw new Error(`postinstall left archives behind: ${leftovers.join(", ")}`);
  }
  console.log(`${PACKAGE}@${values.version} installed from ${values.registry} with its native libraries`);
} finally {
  rmSync(consumerRoot, { recursive: true, force: true });
}
