// The live suite's ldk-server helper signs each request with a request-bound
// macaroon. Check its encoding against ldk-server's reference vectors
// (ldk-server-macaroons/tests/data/macaroons-v2.txt, generated with test keys).
import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import test from "node:test";

import { bindMacaroonToRequest, parseMacaroon } from "./live/ldk.mjs";

// Root key "test-key", identifier "test-id", caveat `permissions = node:read`.
const CREDENTIAL =
  "0201000207746573742d69640002177065726d697373696f6e73203d206e6f64653a7265616400000620fcd6db26ff34eca91e70ec1822ebb0e5ebb0ba3968cdb6544b900d9142a57659";
// CREDENTIAL bound to an empty GetNodeInfo request at Unix time 1800000000.
const BOUND_GET_NODE_INFO =
  "0201000207746573742d69640002177065726d697373696f6e73203d206e6f64653a7265616400026172657175657374203d2031383030303030303030204765744e6f6465496e666f2038383535353038616164653136656335373364323165366134383564666430613736323430383563316131346235656364643634383564653063363833396134000006206553a4b4f42f654b01b195823008752c41d0cff8247b77fbb77e4860debcc7e2";

test("binds a macaroon to a request as ldk-server's reference vector does", () => {
  assert.equal(
    bindMacaroonToRequest(
      CREDENTIAL,
      "GetNodeInfo",
      Buffer.alloc(5),
      1800000000,
    ),
    BOUND_GET_NODE_INFO,
  );
});

test("the bound macaroon chains its signature from the root key", () => {
  const body = Buffer.from([0, 0, 0, 0, 3, 1, 2, 3]);
  const bound = parseMacaroon(
    bindMacaroonToRequest(CREDENTIAL, "Bolt11Send", body),
  );
  const hmac = (key, data) => createHmac("sha256", key).update(data).digest();
  let signature = hmac(
    hmac("macaroons-key-generator", "test-key"),
    bound.identifier,
  );
  for (const caveat of bound.caveats) signature = hmac(signature, caveat);
  assert.deepEqual(signature, bound.signature);
  assert.match(
    bound.caveats.at(-1).toString(),
    /^request = \d+ Bolt11Send [0-9a-f]{64}$/,
  );
});

test("refuses a request-bound macaroon, a bad method and malformed tokens", () => {
  assert.throws(() =>
    bindMacaroonToRequest(BOUND_GET_NODE_INFO, "GetNodeInfo", Buffer.alloc(5)),
  );
  assert.throws(() =>
    bindMacaroonToRequest(
      CREDENTIAL,
      "/api.LightningNode/GetNodeInfo",
      Buffer.alloc(5),
    ),
  );
  assert.throws(() => bindMacaroonToRequest(CREDENTIAL, "", Buffer.alloc(5)));
  assert.throws(() => parseMacaroon("deadbeef"));
  assert.throws(() => parseMacaroon(CREDENTIAL.slice(0, -2)));
  assert.throws(() => parseMacaroon(`${CREDENTIAL}00`));
});
