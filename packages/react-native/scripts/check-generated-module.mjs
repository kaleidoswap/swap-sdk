#!/usr/bin/env node
/**
 * Refuse a turbo module whose glue does not load the generated crate.
 *
 * `ubrn build <platform> --and-generate` renders three files from the list of
 * UniFFI components it finds in the compiled library:
 *
 * - `src/index.tsx`, the package entry, which re-exports the bindings and
 *   calls their `initialize()`;
 * - `cpp/kaleidoswap-swap-sdk.cpp`, whose `installRustCrate` registers the
 *   native module with Hermes;
 * - `android/CMakeLists.txt`, which compiles the generated C++.
 *
 * When that list comes back empty, all three are still written, just with
 * nothing in them, and the build succeeds. The bindings left over from an
 * earlier run (`src/generated`, `cpp/generated`) make the package look
 * complete. 0.11.0 shipped like this: the Android build read a stripped `.so`,
 * which has no symbol table for uniffi's library mode to read component
 * metadata from. Its index exported only `uniffiInitAsync`, so
 * `import { SwapClient } from "@kaleidorg/swap-sdk-react-native"` failed.
 *
 * Run after each generation by `ubrn:fixup`, before packing by
 * `validate-package.mjs`, and against the packed tarball by
 * `smoke-react-native-package.mjs`.
 */
import { readFileSync } from "node:fs";
import { argv, exit } from "node:process";

/** The UniFFI namespace of the `bindings` crate, as ubrn names its files. */
export const NAMESPACE = "kaleidorg_swap_sdk";

/** `Native` + the namespace in upper camel case, as ubrn names the C++ module. */
const CPP_MODULE = "NativeKaleidorgSwapSdk";

const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const BINDINGS = escape(`./generated/${NAMESPACE}`);

/** Each generated glue file, and what it must contain to load the crate. */
export const GLUE = {
  "src/index.tsx": [
    {
      what: "re-export the generated bindings",
      pattern: new RegExp(`^export \\* from ['"]${BINDINGS}['"];?$`, "m"),
    },
    {
      what: "initialize the generated bindings",
      pattern: new RegExp(`\\b${NAMESPACE}\\.default\\.initialize\\(\\)`),
    },
  ],
  "cpp/kaleidoswap-swap-sdk.cpp": [
    {
      what: "register the native module in installRustCrate",
      pattern: new RegExp(`\\b${CPP_MODULE}::registerModule\\(`),
    },
  ],
  "android/CMakeLists.txt": [
    {
      what: "compile the generated C++",
      pattern: new RegExp(`/generated/${NAMESPACE}\\.cpp\\b`),
    },
  ],
};

/**
 * What `source`, the contents of the glue file at `path`, fails to do. Empty
 * when it loads the crate.
 */
export function glueProblems(path, source) {
  const checks = GLUE[path];
  if (checks === undefined) {
    throw new Error(`no checks for ${path}`);
  }
  return checks.filter(({ pattern }) => !pattern.test(source)).map(({ what }) => `${path} does not ${what}`);
}

/**
 * Throw unless every glue file loads the crate. `read(path)` returns a file's
 * contents.
 */
export function assertGlueLoadsCrate(read) {
  const problems = Object.keys(GLUE).flatMap((path) => glueProblems(path, read(path)));
  if (problems.length > 0) {
    throw new Error(
      `the generated turbo module does not load ${NAMESPACE} (${problems.join("; ")}). ` +
        "ubrn found no UniFFI components in the library it generated from; " +
        "check that the library was built with a symbol table (not stripped) and run npm run ubrn:build again",
    );
  }
}

// Only run when invoked directly, so the functions can be imported by the test.
if (import.meta.url === `file://${argv[1]}`) {
  try {
    assertGlueLoadsCrate((path) => readFileSync(path, "utf8"));
    console.log(`the generated turbo module loads ${NAMESPACE}`);
  } catch (error) {
    console.error(error.message);
    exit(1);
  }
}
