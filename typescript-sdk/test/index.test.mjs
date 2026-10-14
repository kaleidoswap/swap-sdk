import assert from "node:assert/strict";
import test from "node:test";

import { isKaleidoSwapError, toJson } from "../dist/index.js";
// The wasm-backed tests below go through the Node entry point, which reads the
// packaged binary from disk — the browser entry's loader `fetch`es it and Node's
// `fetch` refuses `file:` URLs.
import {
  SwapClient,
  SwapWsApi,
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
  timeoutBlockHeight: 123n,
};

const CLAIM_PARAMS = {
  outputAddress:
    "bcrt1p2jln4540qcxyrq024mhnnuc84ye8mra5dyl5fcnql2yl0vukfyesvq7lsr",
  swapId: "swapid",
  keysSecretHex: "11".repeat(32),
  makerBaseUrl: "https://example.invalid",
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
  const client = SwapClient.forNetwork("signet");

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

  const client = SwapClient.forNetwork("signet");
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
  const client = SwapClient.forNetwork("signet");

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
    () => SwapClient.forNetwork({}),
    /argument `network` must be a string/,
  );
  await assertInvalidArgument(
    () => new SwapClient({}),
    /argument `baseUrl` must be a string/,
  );
  await assertInvalidArgument(
    () => new SwapWsApi({}),
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
  const client = SwapClient.forNetwork("signet");
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
  assert.ok(SwapClient.forNetwork("signet") instanceof SwapClient);
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

  assert.ok(client instanceof SwapClient);
  assert.equal(client.apiKeyEnvironment, "test");
  assert.equal(client.apiKeyId, "01KZZYB138E7C3HZX7Q1YBGAQG");

  // There is no accessor for the secret at all: the key crosses into wasm once
  // and JS cannot read it back out. Checked by asking the object for everything
  // it will answer, rather than by listing own properties — a wasm-bindgen
  // instance owns only `__wbg_ptr`, so a property scan can never fail and would
  // pass just as well against a client that exposed the key outright.
  const readable = [
    ...Object.getOwnPropertyNames(client),
    ...Object.getOwnPropertyNames(Object.getPrototypeOf(client)),
  ]
    .filter((name) => name !== "constructor" && name !== "free")
    .map((name) => {
      try {
        const value = client[name];
        return typeof value === "function" ? "" : String(value);
      } catch {
        return "";
      }
    });
  assert.equal(
    readable.some((value) => value.includes("s3cr3t")),
    false,
    `a client property exposed the key secret: ${readable.join(" | ")}`,
  );
  // ...and the scan is worth something: it does see the half that is public.
  assert.equal(
    readable.some((value) => value.includes("01KZZYB138E7C3HZX7Q1YBGAQG")),
    true,
  );

  // The plain constructor authenticates nothing — that is what keeps the client
  // usable against a Boltz maker, which has no notion of an organization key.
  const generic = SwapClient.forNetwork("signet");
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
    }) instanceof SwapClient,
  );
});

test("the options object names the field that is wrong", () => {
  assert.throws(
    () => createKaleidoMakerClient({ apiKey: API_KEY }),
    (error) => isKaleidoSwapError(error) && /makerUrl/.test(error.message),
  );

  // An unknown property is refused rather than ignored. The near misses are
  // `apikey` and `timeout`, and silently dropping the latter leaves a client the
  // caller believes is bounded running with no timeout at all.
  assert.throws(
    () =>
      SwapClient.forKaleidoMaker({
        makerUrl: MAKER_URL,
        apiKey: API_KEY,
        timeout: 30,
      }),
    (error) => isKaleidoSwapError(error) && /timeout/.test(error.message),
  );

  // A document context is a browser, and an organization key in a page is
  // visible to every visitor. §7 of the attribution design says server and
  // native only for this release, and this is what makes the code say it too.
  const hadDocument = "document" in globalThis;
  globalThis.document = {};
  try {
    assert.throws(
      () =>
        SwapClient.forKaleidoMaker({ makerUrl: MAKER_URL, apiKey: API_KEY }),
      (error) => isKaleidoSwapError(error) && /browser/.test(error.message),
    );
    // The error must not quote the key it just refused.
    assert.throws(
      () =>
        SwapClient.forKaleidoMaker({ makerUrl: MAKER_URL, apiKey: API_KEY }),
      (error) => !error.message.includes(API_KEY),
    );
    // Deliberate exceptions stay possible, and have to be written down.
    assert.ok(
      SwapClient.forKaleidoMaker({
        makerUrl: MAKER_URL,
        apiKey: API_KEY,
        allowBrowser: true,
      }) instanceof SwapClient,
    );
    assert.throws(
      () =>
        SwapClient.forKaleidoMaker({
          makerUrl: MAKER_URL,
          apiKey: API_KEY,
          allowBrowser: "yes",
        }),
      (error) =>
        isKaleidoSwapError(error) && /allowBrowser/.test(error.message),
    );
  } finally {
    if (!hadDocument) {
      delete globalThis.document;
    }
  }

  // `timeoutSecs` itself takes a number or a bigint, and nothing else.
  assert.ok(
    SwapClient.forKaleidoMaker({
      makerUrl: MAKER_URL,
      apiKey: API_KEY,
      timeoutSecs: 30,
    }) instanceof SwapClient,
  );
  assert.ok(
    SwapClient.forKaleidoMaker({
      makerUrl: MAKER_URL,
      apiKey: API_KEY,
      timeoutSecs: 30n,
    }) instanceof SwapClient,
  );
  for (const timeoutSecs of [
    -1,
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    // Above 2**64. `as u64` on a float saturates rather than failing, so
    // without a range check this arrived as `u64::MAX` seconds — a deadline
    // tokio never reaches, leaving a client the caller believes is bounded
    // running with no timeout at all.
    1e20,
  ]) {
    assert.throws(
      () =>
        SwapClient.forKaleidoMaker({
          makerUrl: MAKER_URL,
          apiKey: API_KEY,
          timeoutSecs,
        }),
      (error) => isKaleidoSwapError(error) && /timeoutSecs/.test(error.message),
      `timeoutSecs: ${timeoutSecs} should have been rejected`,
    );
  }
});

// A rejected call must name the property and never quote its contents.
//
// The options object is deserialized by hand for this reason. Routing it
// through serde-wasm-bindgen put the *value* in the message — so
// `forKaleidoMaker(process.env.KALEIDOSWAP_API_KEY)`, the transposed call the
// named-options shape exists to catch, threw `invalid type: string
// "kld_live_…"` and handed a live organization key to whatever caught it.
test("a mistyped options argument never quotes the key back", () => {
  const mistakes = [
    // The whole key where the options object belongs.
    () => SwapClient.forKaleidoMaker(API_KEY),
    // A wrapped key, the shape a config object would produce.
    () =>
      SwapClient.forKaleidoMaker({
        makerUrl: MAKER_URL,
        apiKey: { key: API_KEY },
      }),
    () =>
      SwapClient.forKaleidoMaker({ makerUrl: MAKER_URL, apiKey: [API_KEY] }),
    // Arguments the other way round.
    () => SwapClient.forKaleidoMaker({ makerUrl: API_KEY, apiKey: MAKER_URL }),
  ];

  for (const attempt of mistakes) {
    assert.throws(attempt, (error) => {
      const rendered = `${error.message} ${error.stack ?? ""}`;
      assert.equal(
        rendered.includes("s3cr3t"),
        false,
        `the key secret reached the error: ${error.message}`,
      );
      return isKaleidoSwapError(error);
    });
  }
});

// ---------------------------------------------------------------------------
// Arkade Intents corridor.
// ---------------------------------------------------------------------------

import {
  IntentsCorridor,
  RFQ_TERMINAL_STATES,
  corridorRootFromMakerUrl,
  isRfqQuote,
  newRfqId,
} from "../dist/index.node.js";

// The TS rule and the Rust rule (behind the wasm `corridorUrl` getter) must
// agree on every input, or the main entry and the `./arkade` venue would reach
// two different origins from one `makerUrl`.
test("the corridor root is the origin the /v2 base hangs off, in TS and in wasm alike", () => {
  for (const [base, root] of [
    [
      "https://maker.signet.kaleidoswap.com/v2",
      "https://maker.signet.kaleidoswap.com",
    ],
    [
      "https://maker.signet.kaleidoswap.com/v2/",
      "https://maker.signet.kaleidoswap.com",
    ],
    ["http://localhost:9001/v2", "http://localhost:9001"],
    ["https://host/prefix/v2", "https://host/prefix"],
  ]) {
    assert.equal(corridorRootFromMakerUrl(base), root, base);
    assert.equal(new SwapClient(base).corridorUrl, root, `wasm: ${base}`);
    assert.equal(new IntentsCorridor(new SwapClient(base)).url, root);
  }
  for (const bad of [
    "https://maker.example",
    "https://maker.example/v1",
    "https://maker.example/v2?x=1",
    "https://maker.example/v2#frag",
  ]) {
    assert.throws(() => corridorRootFromMakerUrl(bad), bad);
    assert.throws(() => new SwapClient(bad).corridorUrl, `wasm: ${bad}`);
  }
});

// `ARKD` is a symbol the maker publishes, so a caller reading the catalogue
// arrives with a route it was shown. The refusal has to say where it lives.
test("ARKD on a Boltz-shaped create is refused before any I/O, pointing at the corridor", async () => {
  const client = SwapClient.forNetwork("signet");
  await assert.rejects(
    client.createReverseSwap("signet", {
      from: "BTC",
      to: "ARKD",
      preimageHash: "00".repeat(32),
      claimPublicKey: PUBKEY,
      invoiceAmount: 12000n,
    }),
    (error) =>
      isKaleidoSwapError(error) &&
      error.code === "InvalidArgument" &&
      /Intents corridor/.test(error.message) &&
      /quoteLightningReceive/.test(error.message),
  );
});

test("newRfqId is 32 random bytes of hex, fresh each call", () => {
  const a = newRfqId();
  const b = newRfqId();
  assert.match(a, /^[0-9a-f]{64}$/);
  assert.notEqual(a, b);
});

test("isRfqQuote narrows on the wire type, and the terminal set matches the SDK's", () => {
  const quote = {
    v: 1,
    type: "rfq_quote",
    rfq_id: "x",
    pair: "arkade:BTC->lightning:BTC",
    from_amount: 12120n,
    to_amount: 12000n,
    solver_pubkey: "02ab",
    valid_until: 1n,
    profile: {},
  };
  const refusal = {
    v: 1,
    type: "rfq_refusal",
    rfq_id: "x",
    reason: "exposure_cap",
  };
  assert.equal(isRfqQuote(quote), true);
  assert.equal(isRfqQuote(refusal), false);
  assert.deepEqual([...RFQ_TERMINAL_STATES].sort(), [
    "expired",
    "refunded",
    "refused",
    "settled",
    "stuck",
  ]);
});

// A clean break rather than an alias: nothing external consumes this SDK, and
// every internal consumer sits on ^0.7.0, which a 0.x caret range never carries
// across a minor. Leaving the old names exported would have kept the vocabulary
// we are removing alive in new code for no one's benefit.
test("the Boltz-derived names are gone from the public surface", async () => {
  const surface = await import("../dist/index.node.js");
  assert.equal(typeof surface.SwapClient, "function");
  assert.equal(typeof surface.SwapWsApi, "function");
  assert.equal(typeof surface.SwapWsUpdates, "function");
  for (const removed of ["BoltzClient", "BoltzWsApi", "BoltzWsUpdates"]) {
    assert.equal(surface[removed], undefined, `${removed} is still exported`);
  }
});

test("the renamed client carries the static constructors", () => {
  assert.equal(typeof SwapClient.forNetwork, "function");
  assert.equal(typeof SwapClient.forKaleidoMaker, "function");
  assert.equal(
    SwapClient.forNetwork("signet").corridorUrl,
    "https://maker.signet.kaleidoswap.com",
  );
});

// A BTC reverse swap built from fixed test keys (`11…`, ours; `22…`, the
// maker's) and preimage `33…`; the same values as the Rust test
// `btc_claim_spends_the_existing_lockup_tx`. Its lockup pays an unrelated
// output first and the 50,000-sat swap output second.
const LOCKUP_REVERSE = {
  id: "reverse",
  swapTree: {
    claimLeaf: {
      version: 192,
      output:
        "82012088a914cfb21d36b8551e32a32a4331a394460d7c43a90a88204f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aaac",
    },
    refundLeaf: {
      version: 192,
      output:
        "20466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27ad02c800b1",
    },
  },
  lockupAddress:
    "bcrt1ppm80f8gx33wmfw4m233axe9tmrsvawh7ser8y9m7q973hd89khhsz6v2q0",
  refundPublicKey:
    "02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27",
  timeoutBlockHeight: 200,
  onchainAmount: 50000n,
};
const LOCKUP_OUR_PUBKEY =
  "034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
const LOCKUP_TXID =
  "90c0533cd7108084f09d7f6fb7f9eef9f36a99489e2a533a7a38d2acebe70119";
const LOCKUP_TX_HEX =
  "020000000144444444444444444444444444444444444444444444444444444444444444440000000000ffffffff027011010000000000160014555555555555555555555555555555555555555550c30000000000002251200ecef49d068c5db4babb5463d364abd8e0cebafe864672177e017d1bb4e5b5ef00000000";

/** The first input's previous outpoint of a serialized segwit transaction. */
function firstPrevout(txHex) {
  // version (4 bytes), segwit marker and flag (2), input count (1).
  const txid = txHex.slice(14, 78).match(/../g).reverse().join("");
  const vout = Buffer.from(txHex.slice(78, 86), "hex").readUInt32LE(0);
  return `${txid}:${vout}`;
}

function lockupReverseScript() {
  return SwapScript.fromReverse(
    "bitcoin",
    "regtest",
    LOCKUP_REVERSE,
    LOCKUP_OUR_PUBKEY,
  );
}

test("constructClaim spends the claimLockup and asks nobody where it is", async () => {
  const { createServer } = await import("node:http");
  const requests = [];
  const server = createServer((req, res) => {
    requests.push(`${req.method} ${req.url}`);
    req.resume();
    req.on("end", () => {
      res.statusCode = 500;
      res.end("unexpected request");
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const claim = await lockupReverseScript().constructClaim("33".repeat(32), {
      ...CLAIM_PARAMS,
      swapId: "reverse",
      // Nothing listens here: a maker request fails the claim.
      makerBaseUrl: "http://127.0.0.1:1/v2",
      bitcoinEsploraUrl: `http://127.0.0.1:${server.address().port}`,
      cooperative: false,
      feeAbsoluteSat: 500n,
      claimLockup: { chain: "bitcoin", hex: LOCKUP_TX_HEX, txid: LOCKUP_TXID },
    });

    assert.equal(firstPrevout(claim.hex()), `${LOCKUP_TXID}:1`);
    // Pinned and never rebroadcast: no Esplora request at all.
    assert.deepEqual(requests, []);
  } finally {
    server.close();
  }
});

test("a claimLockup whose hex does not match its txid is rejected", async () => {
  await assertInvalidArgument(
    () =>
      lockupReverseScript().constructClaim("33".repeat(32), {
        ...CLAIM_PARAMS,
        claimLockup: {
          chain: "bitcoin",
          hex: LOCKUP_TX_HEX,
          txid: "00".repeat(32),
        },
      }),
    /claimLockup hex does not match its trusted txid/,
  );
});

test("a claimLockup on an unknown chain is rejected", async () => {
  await assertInvalidArgument(
    () =>
      lockupReverseScript().constructClaim("33".repeat(32), {
        ...CLAIM_PARAMS,
        claimLockup: { chain: "ark", hex: LOCKUP_TX_HEX, txid: LOCKUP_TXID },
      }),
    /claimLockup.chain must be bitcoin or liquid/,
  );
});

test("a Bitcoin claimLockup labelled liquid does not decode", async () => {
  await assert.rejects(() =>
    lockupReverseScript().constructClaim("33".repeat(32), {
      ...CLAIM_PARAMS,
      claimLockup: { chain: "liquid", hex: LOCKUP_TX_HEX, txid: LOCKUP_TXID },
    }),
  );
});

// `makerBaseUrl` is renamed in this layer only — the wasm boundary still
// deserializes `boltzBaseUrl`, so the mapping is load-bearing and silent when
// wrong: a dropped field would surface as a confusing error from a later step.
test("makerBaseUrl reaches the binding under the name it deserializes", async () => {
  const script = SwapScript.fromSubmarine(
    "bitcoin",
    "regtest",
    SUBMARINE_RESPONSE,
    PUBKEY,
  );

  // Reaching the preimage check proves the params object deserialized, which
  // it cannot do without the URL field under its wasm-side name.
  await assertInvalidArgument(
    () => script.constructClaim("zz", CLAIM_PARAMS),
    /argument `preimageHex` is not a hex preimage/,
  );

  // The old spelling is ignored, and saying so is the point: the binding's own
  // answer is "invalid type: unit value, expected a string", which names no
  // field and reads like an SDK bug rather than a renamed key.
  const { makerBaseUrl, ...withoutUrl } = CLAIM_PARAMS;
  await assertInvalidArgument(
    () =>
      script.constructClaim("11".repeat(32), {
        ...withoutUrl,
        boltzBaseUrl: makerBaseUrl,
      }),
    /`makerBaseUrl` is required.*named `boltzBaseUrl` before 0\.9\.0/s,
  );
});

test("account restore discovers records and indices through WASM without applying the root path", async () => {
  const { createServer } = await import("node:http");
  const master = SwapMasterKey.fromWalletMnemonic(MNEMONIC, "regtest");
  const requests = [];
  const restored = {
    id: "01KZZYB138E7C3HZX7Q1YBGAQG",
    type: "submarine",
    status: "invoice.set",
    createdAt: 1800000000,
    from: "BTC",
    to: "BTC",
  };
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    const request = JSON.parse(body);
    requests.push(request);
    // Model the maker's discovery boundary: an account xpub only finds its
    // direct child keys with "m"; the root-xpub default finds nothing.
    const found =
      request.xpub === master.masterXpub() && request.derivationPath === "m";
    res.setHeader("content-type", "application/json");
    res.end(
      JSON.stringify(
        req.url.endsWith("/index")
          ? { index: found ? 3 : -1 }
          : found
            ? [restored]
            : [],
      ),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = new SwapClient(`http://127.0.0.1:${server.address().port}/v2`);
  try {
    assert.deepEqual(await client.swapRestore(master.masterXpub()), []);
    const swaps = await master.restore(client, 50);
    assert.equal(swaps[0].id, restored.id);
    assert.equal(swaps[0].createdAt, 1800000000n);
    assert.equal((await master.restoreIndex(client, 50)).index, 3n);
    assert.equal((await master.restore(client)).length, 1);
    assert.equal((await master.restoreIndex(client)).index, 3n);
    assert.ok(requests.slice(1).every((r) => r.derivationPath === "m"));
    assert.equal(requests[1].gapLimit, 50);
    assert.equal(requests[2].gapLimit, 50);
    assert.equal(requests[3].gapLimit, undefined);
    assert.equal(requests[4].gapLimit, undefined);
    assert.ok(requests.every((r) => !JSON.stringify(r).includes(MNEMONIC)));
  } finally {
    client.free();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("restored keys recover swapAuth through WASM and authorize a re-quote", async () => {
  const { createServer } = await import("node:http");
  const id = "01KZZYB138E7C3HZX7Q1YBGAQG";
  const challenge = Buffer.alloc(72, 42);
  challenge.writeBigInt64BE(1800000300n);
  const auth = "a1".repeat(32);
  const requests = [];
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    requests.push({
      url: req.url,
      headers: req.headers,
      body: JSON.parse(body),
    });
    res.setHeader("content-type", "application/json");
    if (req.url.endsWith("/auth/challenge")) {
      res.end(
        JSON.stringify({
          challenge: challenge.toString("hex"),
          expiresAt: 1800000300,
        }),
      );
    } else if (req.url.endsWith("/auth/recover")) {
      res.end(JSON.stringify({ swapAuth: auth }));
    } else {
      res.end("{}");
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = new SwapClient(`http://127.0.0.1:${server.address().port}/v2`);
  try {
    const keys = SwapMasterKey.fromWalletMnemonic(
      MNEMONIC,
      "regtest",
    ).deriveSwapKey(3n);
    const recovered = await client.recoverSwapAuth(id, keys.secretKey);
    assert.equal(recovered, auth);
    await client.acceptQuote(id, 93500n, recovered);
    assert.deepEqual(
      requests.map((r) => r.url),
      [
        `/v2/swap/${id}/auth/challenge`,
        `/v2/swap/${id}/auth/recover`,
        `/v2/swap/chain/${id}/quote`,
      ],
    );
    assert.equal(requests[1].body.challenge, challenge.toString("hex"));
    assert.match(requests[1].body.signature, /^[0-9a-f]{128}$/);
    assert.equal(requests[2].headers["x-swap-auth"], recovered);
    assert.equal(requests[2].body.amount, 93500);
    assert.ok(
      requests.every((r) => !JSON.stringify(r.body).includes(keys.secretKey)),
    );
  } finally {
    client.free();
    await new Promise((resolve) => server.close(resolve));
  }
});
