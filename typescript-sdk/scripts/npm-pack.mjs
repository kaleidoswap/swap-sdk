import { execFileSync } from "node:child_process";

/**
 * `npm pack --json` in `cwd`, as the one packed tarball's entry
 * (`{ filename, files, ... }`).
 *
 * npm 12 changed the output's shape (npm/cli#9247): it was an array of
 * entries and is now an object keyed by package name, like `npm publish
 * --json`. The entries themselves did not change. Both shapes are read here,
 * so the smoke tests run under any npm a Node release ships.
 */
export function packJson(cwd) {
  const output = JSON.parse(
    execFileSync("npm", ["pack", "--json"], { cwd, encoding: "utf8" }),
  );
  const entries = Array.isArray(output) ? output : Object.values(output);
  if (entries.length !== 1 || typeof entries[0]?.filename !== "string") {
    throw new Error(
      `npm pack --json did not describe exactly one tarball: ${JSON.stringify(output).slice(0, 200)}`,
    );
  }
  return entries[0];
}
