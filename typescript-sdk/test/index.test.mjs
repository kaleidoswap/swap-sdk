import assert from "node:assert/strict";
import test from "node:test";

import { isKaleidoSwapError, toJson } from "../dist/index.js";

test("toJson preserves bigint amounts as decimal strings", () => {
  assert.equal(
    toJson({ settled: 42n, nested: [1n, 2n] }),
    '{"settled":"42","nested":["1","2"]}',
  );
});

test("isKaleidoSwapError requires an Error with a stable code", () => {
  assert.equal(
    isKaleidoSwapError(
      Object.assign(new Error("failed"), { code: "SWAP_FAILED" }),
    ),
    true,
  );
  assert.equal(isKaleidoSwapError({ code: "SWAP_FAILED" }), false);
  assert.equal(isKaleidoSwapError(new Error("failed")), false);
});
