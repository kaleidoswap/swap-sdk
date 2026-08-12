import assert from "node:assert/strict";
import test from "node:test";

import { isKaleidoSwapError, toJson } from "../dist/index.js";
import { init, SwapMasterKey } from "../dist/index.node.js";

// A published test vector, not a wallet: BIP39's all-`abandon` mnemonic.
const MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

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

// Derived values must reach JS as plain objects, reachable by property. They used
// to cross as `Map`s, which reads `undefined` for every declared field without
// raising anything — `derivePreimage(i).sha256` in particular, whose value is the
// `preimageHash` a swap is created with, so the mistake surfaced as the maker
// rejecting a request rather than as anything pointing here. Asserting the field
// values is not enough on its own: a `Map` would fail these as `undefined`, but
// the `instanceof` checks are what name the actual contract.
test("derived keys and preimages cross as plain objects, not Maps", async () => {
  await init();
  const master = SwapMasterKey.fromSwapMnemonic(MNEMONIC, "signet");

  const key = master.deriveSwapKey(0n);
  assert.equal(key instanceof Map, false);
  assert.match(key.publicKey, /^0[23][0-9a-f]{64}$/);
  assert.match(key.secretKey, /^[0-9a-f]{64}$/);

  const preimage = master.derivePreimage(0n);
  assert.equal(preimage instanceof Map, false);
  assert.match(preimage.preimage, /^[0-9a-f]{64}$/);
  assert.match(preimage.sha256, /^[0-9a-f]{64}$/);
  assert.match(preimage.hash160, /^[0-9a-f]{40}$/);
});
