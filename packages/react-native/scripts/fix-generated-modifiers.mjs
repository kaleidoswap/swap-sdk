#!/usr/bin/env node
/**
 * Reorder `async public` to `public async` in the generated TypeScript.
 *
 * uniffi-bindgen-react-native 0.29.3-1 emits every async method on an object
 * with its modifiers the wrong way round, which is a syntax error:
 *
 *     async public getHeight(): Promise<HeightResponse>
 *     TS1029: 'public' modifier must precede 'async' modifier.
 *
 * The cause is in the generator's own template. `func_decl` renders
 * `{{ prefix }}{% call async_kw %}{{ func_decl }} {{ name }}`, which is the
 * right shape — visibility, then `async`, then any declaration keyword — but
 * `method_decl` forwards its argument into the `func_decl` slot rather than
 * `prefix`, so `ObjectTemplate`'s "public" lands after the `async`. Top-level
 * functions are unaffected: they pass `export function` as the declaration
 * keyword and an empty prefix, which renders correctly.
 *
 * Nearly every method in this SDK is an async method on an object, so this is
 * the whole surface rather than an edge case.
 *
 * Fixed here rather than by patching the generator because this repository
 * cannot build Rust locally, so a template patch could only be tested by a
 * full CI round-trip, while this is checked by `npm test` in seconds. It
 * should go upstream and this should then be deleted.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { argv, exit } from "node:process";

/** `async public foo(` -> `public async foo(`, and the `public static` form. */
export const MISORDERED = /\basync (public(?: static)?) /g;

export function reorderModifiers(source) {
  return source.replace(MISORDERED, "$1 async ");
}

function main(files) {
  if (files.length === 0) {
    console.error("usage: fix-generated-modifiers.mjs <file...>");
    return 1;
  }
  for (const file of files) {
    const before = readFileSync(file, "utf8");
    const after = reorderModifiers(before);
    if (before === after) {
      // Not an error: the generator may have been fixed, or this file may have
      // no async methods. The typecheck that follows is what actually decides.
      console.log(`no misordered modifiers in ${file}`);
      continue;
    }
    const count = (before.match(MISORDERED) ?? []).length;
    writeFileSync(file, after);
    console.log(`reordered ${count} async/public modifier pair(s) in ${file}`);
  }
  return 0;
}

// Only run when invoked directly, so the regex can be imported by the test.
if (import.meta.url === `file://${argv[1]}`) {
  exit(main(argv.slice(2)));
}
