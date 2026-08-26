import assert from "node:assert/strict";
import test from "node:test";

import { isKaleidoSwapError, toJson } from "../dist/index.js";
// The wasm-backed tests below go through the Node entry point, which reads the
// packaged binary from disk — the browser entry's loader `fetch`es it and Node's
// `fetch` refuses `file:` URLs.
import {
  BoltzClient,
  BoltzWsApi,
  createKaleidoMakerClient,
  init,
  SwapMasterKey,
  SwapScript,
} from "../dist/index.node.js";

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

// ---------------------------------------------------------------------------
// Rejections at the wasm boundary.
//
// Every one of these calls is rejected before any network I/O: the checks under
// test run on the arguments themselves.
// ---------------------------------------------------------------------------

await init();

const MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/** A public key that is actually on the curve, for the `response` arguments. */
const PUBKEY = SwapMasterKey.fromWalletMnemonic(
  MNEMONIC,
  "regtest",
).deriveSwapKey(0n).publicKey;

/**
 * A submarine create response with every required field present, so that
 * deserialization succeeds and the checks after it are the ones under test. The
 * swap tree is a real one (`tests/fixtures/lusdt-v1/wire-contract.json`); nothing
 * here reconstructs a script, so it needs to satisfy serde and no more.
 */

// Derived values must reach JS as plain objects, reachable by property. They used
// to cross as `Map`s, which reads `undefined` for every declared field without
// raising anything — `derivePreimage(i).sha256` in particular, whose value is the
// `preimageHash` a swap is created with, so the mistake surfaced as the maker
// rejecting a request rather than as anything pointing here.
test("derived keys and preimages cross as plain objects, not Maps", () => {
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

const SUBMARINE_RESPONSE = {
  acceptZeroConf: true,
  address: "bcrt1p2jln4540qcxyrq024mhnnuc84ye8mra5dyl5fcnql2yl0vukfyesvq7lsr",
  bip21:
    "bitcoin:bcrt1p2jln4540qcxyrq024mhnnuc84ye8mra5dyl5fcnql2yl0vukfyesvq7lsr",
  claimPublicKey: PUBKEY,
  expectedAmount: 100000n,
  id: "swapid",
  swapTree: {
    claimLeaf: {
      version: 196,
      output:
        "a914124b4a204760441dc802c445d4987ba1bd967e6d882083d1d7b47cd4163db23e633b81a3a6906a99a99e5acbbe4df29172f42621668cac",
    },
    refundLeaf: {
      version: 196,
      output:
        "2010154f49ec6656fd70d28abd9bbb71633da124eda324b1bf2b28dd9686915087ad017bb1",
    },
  },
  timeoutBlockHeight: 100n,
};

const CLAIM_PARAMS = {
  outputAddress:
    "bcrt1p2jln4540qcxyrq024mhnnuc84ye8mra5dyl5fcnql2yl0vukfyesvq7lsr",
  swapId: "swapid",
  keysSecretHex: "11".repeat(32),
  boltzBaseUrl: "https://example.invalid",
  network: "regtest",
  bitcoinEsploraUrl: "https://example.invalid",
  feeAbsoluteSat: 100n,
};

/**
 * Assert that `fn` rejects with an `InvalidArgument` error matching `message`.
 *
 * Checks the shape as well as the text: a bare string thrown across the boundary
 * has no `.message` and fails `instanceof Error`, and a wasm trap is a
 * `RuntimeError` whose message names neither the argument nor the field.
 */
async function assertInvalidArgument(fn, message) {
  let caught;
  try {
    await fn();
  } catch (error) {
    caught = error;
  }
  assert.ok(caught !== undefined, "expected a rejection");
  assert.ok(
    caught instanceof Error,
    `expected an Error, got ${typeof caught}: ${String(caught)}`,
  );
  assert.notEqual(caught.constructor.name, "RuntimeError");
  assert.equal(isKaleidoSwapError(caught), true);
  assert.equal(caught.code, "InvalidArgument");
  assert.match(caught.message, message);
}

test("a malformed request object names the missing field", async () => {
  const client = BoltzClient.forNetwork("signet");

  await assertInvalidArgument(
    () => client.createReverseSwap("signet", { onchainAmount: 100000 }),
    /missing field `from`/,
  );
  await assertInvalidArgument(
    () => client.createSubmarineSwap("signet", {}),
    /missing field `from`/,
  );
  await assertInvalidArgument(
    () => client.createChainSwap("signet", { from: "BTC" }),
    /missing field `to`/,
  );
  await assertInvalidArgument(
    () => SwapScript.fromSubmarine("bitcoin", "regtest", { id: "x" }, PUBKEY),
    /missing field `acceptZeroConf`/,
  );
  await assertInvalidArgument(
    () => SwapScript.fromReverse("bitcoin", "regtest", { id: "x" }, PUBKEY),
    /missing field `swapTree`/,
  );
});

test("a mistyped request field names the type it expected", async () => {
  await assertInvalidArgument(
    () =>
      SwapScript.fromSubmarine(
        "bitcoin",
        "regtest",
        { acceptZeroConf: true, address: "a", bip21: "b", expectedAmount: "x" },
        PUBKEY,
      ),
    /invalid type: string "x", expected u64/,
  );

  const client = BoltzClient.forNetwork("signet");
  await assertInvalidArgument(
    () => client.createReverseSwap("signet", null),
    /expected struct CreateReverseRequest/,
  );
});

// wasm-bindgen marshals `string` parameters in its generated JS glue before any
// Rust code runs, and given a non-string that glue traps in the wasm allocator
// with `RuntimeError: memory access out of bounds` — which names neither the
// argument nor the call. Passing arguments in the wrong order is the usual way to
// hit it, and TypeScript cannot catch it for a plain-JS caller.
test("a non-string where a string is required names the argument", async () => {
  const client = BoltzClient.forNetwork("signet");

  await assertInvalidArgument(
    () => client.createReverseSwap({ onchainAmount: 100000 }, "signet"),
    /argument `network` must be a string/,
  );
  await assertInvalidArgument(
    () => SwapScript.fromSubmarine({ id: "x" }, "bitcoin", PUBKEY),
    /argument `chainKind` must be a string/,
  );
  await assertInvalidArgument(
    () => client.swap(123),
    /argument `swapId` must be a string/,
  );
  await assertInvalidArgument(
    () => BoltzClient.forNetwork({}),
    /argument `network` must be a string/,
  );
  await assertInvalidArgument(
    () => new BoltzClient({}),
    /argument `baseUrl` must be a string/,
  );
  await assertInvalidArgument(
    () => new BoltzWsApi({}),
    /argument `wsUrl` must be a string/,
  );
  await assertInvalidArgument(
    () => SwapMasterKey.fromWalletMnemonic({}, "regtest"),
    /argument `walletMnemonic` must be a string/,
  );
  // An optional string argument, supplied but not a string.
  await assertInvalidArgument(
    () => client.swapRestore("xpub", {}),
    /argument `derivationPath` must be a string/,
  );
});

// The upstream key parsers render some failures as the bare string "string
// error", which says neither which argument was wrong nor why.
test("an unparseable key argument names the argument", async () => {
  await assertInvalidArgument(
    () =>
      SwapScript.fromSubmarine("bitcoin", "regtest", SUBMARINE_RESPONSE, "zz"),
    /argument `ourPubkeyHex` is not a hex public key/,
  );
  await assertInvalidArgument(
    () =>
      SwapScript.fromSubmarine(
        "bitcoin",
        "regtest",
        SUBMARINE_RESPONSE,
        // Well-formed hex of the right length, but not a point on the curve —
        // the case the upstream parser reports as "string error".
        "02" + "11".repeat(32),
      ),
    /argument `ourPubkeyHex` is not a hex public key/,
  );
});

test("an unparseable claim preimage names preimageHex", async () => {
  const script = SwapScript.fromSubmarine(
    "bitcoin",
    "regtest",
    SUBMARINE_RESPONSE,
    PUBKEY,
  );

  await assertInvalidArgument(
    () => script.constructClaim("zz", CLAIM_PARAMS),
    /argument `preimageHex` is not a hex preimage/,
  );
});

test("an omitted optional string argument stays absent", () => {
  // `Option<StringArg>` has to keep treating null/undefined as "not supplied"
  // rather than as a non-string to reject.
  const withPassphrase = SwapMasterKey.fromWalletMnemonic(
    MNEMONIC,
    "regtest",
    "passphrase",
  );
  const omitted = SwapMasterKey.fromWalletMnemonic(MNEMONIC, "regtest");
  const explicitUndefined = SwapMasterKey.fromWalletMnemonic(
    MNEMONIC,
    "regtest",
    undefined,
  );

  assert.equal(omitted.masterXpub(), explicitUndefined.masterXpub());
  assert.notEqual(omitted.masterXpub(), withPassphrase.masterXpub());
});

test("the wasm instance stays usable after a rejected call", async () => {
  const client = BoltzClient.forNetwork("signet");
  await assert.rejects(() =>
    client.createReverseSwap({ onchainAmount: 100000 }, "signet"),
  );

  // A trap would have left the instance in an undefined state; a rejection does
  // not, so ordinary calls still work afterwards.
  assert.equal(
    SwapMasterKey.fromWalletMnemonic(MNEMONIC, "regtest").deriveSwapKey(0n)
      .publicKey,
    PUBKEY,
  );
  assert.ok(BoltzClient.forNetwork("signet") instanceof BoltzClient);
});

// ---------------------------------------------------------------------------
// Partner attribution: the organization API key.
// ---------------------------------------------------------------------------

const API_KEY = "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz";
const MAKER_URL = "https://maker.signet.kaleidoswap.com/v2";

test("a Kaleido maker client exposes the key's public half and not its secret", () => {
  const client = createKaleidoMakerClient({
    makerUrl: MAKER_URL,
    apiKey: API_KEY,
  });

  assert.ok(client instanceof BoltzClient);
  assert.equal(client.apiKeyEnvironment, "test");
  assert.equal(client.apiKeyId, "01KZZYB138E7C3HZX7Q1YBGAQG");

  // There is no accessor for the secret at all: the key crosses into wasm once
  // and JS cannot read it back out.
  assert.equal(
    JSON.stringify(Object.getOwnPropertyNames(client)).includes("s3cr3t"),
    false,
  );

  // The plain constructor authenticates nothing — that is what keeps the client
  // usable against a Boltz maker, which has no notion of an organization key.
  const generic = BoltzClient.forNetwork("signet");
  assert.equal(generic.apiKeyEnvironment, undefined);
  assert.equal(generic.apiKeyId, undefined);
});

test("a value that cannot be an organization key is rejected locally", () => {
  for (const apiKey of [
    "",
    "sk_test_abc_secret",
    "kld_staging_abc_secret",
    "kld_test_abc",
  ]) {
    assert.throws(
      () => createKaleidoMakerClient({ makerUrl: MAKER_URL, apiKey }),
      // A `401` from the maker is what a *revoked* key gets, so a local typo
      // must not arrive looking like one.
      (error) => isKaleidoSwapError(error),
      `"${apiKey}" should not have been accepted`,
    );
  }
});

test("an organization key is refused a maker it must not be sent to", () => {
  // Plain HTTP to a remote host: a bearer credential anything on the path can
  // read, and the key is permanent until revoked.
  assert.throws(() =>
    createKaleidoMakerClient({
      makerUrl: "http://maker.signet.kaleidoswap.com/v2",
      apiKey: API_KEY,
    }),
  );

  // Loopback is the regtest harness, where the "network" is a socket on this
  // machine.
  assert.ok(
    createKaleidoMakerClient({
      makerUrl: "http://127.0.0.1:9001/v2",
      apiKey: API_KEY,
    }) instanceof BoltzClient,
  );
});

test("the options object names the field that is wrong", () => {
  assert.throws(
    () => createKaleidoMakerClient({ apiKey: API_KEY }),
    (error) => isKaleidoSwapError(error) && /makerUrl/.test(error.message),
  );
});
