#!/usr/bin/env node
/**
 * Make the generated `SwapClient.kaleidoMaker` refuse to run without an
 * explicit `{ allowMobile: true }`.
 *
 * `kaleidoMaker` takes a partner organization's secret `kld_` key, and this
 * package only runs inside a mobile app, where the key would be readable by
 * every user. The guard cannot go in the Rust `bindings` crate, because that
 * crate also builds the Python wheel, where a server holding the key is the
 * intended use. So after each `ubrn` generation this script:
 *
 * - imports `kaleidoMakerRefusal` from `src/kaleido-maker-guard.ts`, and
 *   re-exports its options type so callers can name it;
 * - adds an optional fourth parameter, `options?: KaleidoMakerAppOptions`;
 * - throws `Exception.Generic` with the refusal before the native call.
 *
 * Unlike `fix-generated-modifiers.mjs`, a file it cannot patch is an error, not
 * a no-op. If the generator renames the method or changes its shape, building
 * on would ship the constructor unguarded, so the build stops here instead.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { argv, exit } from "node:process";

/** `public static kaleidoMaker(<params>)<return type> {`, as ubrn renders it. */
const SIGNATURE = /(public static kaleidoMaker\()([^)]*)(\)[^{]*\{)/g;

const IMPORT =
  'import { kaleidoMakerRefusal, type KaleidoMakerAppOptions } from "../kaleido-maker-guard";\n' +
  'export type { KaleidoMakerAppOptions } from "../kaleido-maker-guard";\n';

const PARAMETER = "options?: KaleidoMakerAppOptions";

const REFUSAL =
  "\n    const refusal = kaleidoMakerRefusal(options);" +
  "\n    if (refusal !== undefined) throw new Exception.Generic(refusal);";

/** Whether `source` already carries the guard this script adds. */
export function isGuarded(source) {
  return source.includes(IMPORT) && source.includes(PARAMETER) && source.includes(REFUSAL);
}

/**
 * The patched source. Idempotent: an already guarded file comes back
 * unchanged. Throws when `kaleidoMaker` is missing or appears more than once.
 */
export function guardKaleidoMaker(source) {
  if (isGuarded(source)) {
    return source;
  }
  if ([IMPORT, PARAMETER, REFUSAL].some((part) => source.includes(part))) {
    throw new Error("the generated file is partly guarded; regenerate it rather than patch it twice");
  }
  const matches = [...source.matchAll(SIGNATURE)];
  if (matches.length !== 1) {
    throw new Error(
      `expected exactly one generated \`public static kaleidoMaker(\`, found ${matches.length}; ` +
        "the generator's output changed, and the in-app guard must be re-applied by hand",
    );
  }
  const firstImport = source.search(/^import /m);
  if (firstImport === -1) {
    throw new Error("found no import in the generated file to place the guard's import beside");
  }
  const guarded = source.replace(
    SIGNATURE,
    (_, open, params, close) => `${open}${params}, ${PARAMETER}${close}${REFUSAL}`,
  );
  return guarded.slice(0, firstImport) + IMPORT + guarded.slice(firstImport);
}

function main(files) {
  if (files.length === 0) {
    console.error("usage: guard-kaleido-maker.mjs <file...>");
    return 1;
  }
  for (const file of files) {
    const before = readFileSync(file, "utf8");
    const after = guardKaleidoMaker(before);
    if (before === after) {
      console.log(`kaleidoMaker is already guarded in ${file}`);
      continue;
    }
    writeFileSync(file, after);
    console.log(`guarded kaleidoMaker in ${file}`);
  }
  return 0;
}

// Only run when invoked directly, so the functions can be imported by the test.
if (import.meta.url === `file://${argv[1]}`) {
  try {
    exit(main(argv.slice(2)));
  } catch (error) {
    console.error(error.message);
    exit(1);
  }
}
