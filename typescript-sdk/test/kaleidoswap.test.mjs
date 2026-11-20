// Tests for the high-level surface on the TypeScript side. The phase machine
// itself — pricing, validation, persistence order, claim and refund, resume —
// is the Rust core's and is tested there against a fake maker
// (`src/swaps/kaleidoswap.rs`). Here: the wasm binding end to end for the
// paths that need no valid swap tree (quote, and the checks `start` makes
// before it contacts the maker), the adapters, and the Node stores.
import assert from "node:assert/strict";
import test from "node:test";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { MemoryStore, isQuote, isSwapError } from "../dist/kaleidoswap.js";
import {
  FileStore,
  SwapMasterKey,
  createKaleidoSwap,
  init,
  mnemonicSigner,
} from "../dist/index.node.js";

// A hold invoice the signet maker issued (50 000 sat, long expired), and a
// regtest one.
const INVOICE_SIGNET_50K =
  "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e";
const INVOICE_REGTEST_100K =
  "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysyt5fzeueg687mkcum4we2pm452clxqdy9l4extcqs84ek4nte3s56s6pmwxhmx4zx3d4k6xsftlphw8ed60xerfchczpw8w8avdn92qpftvz36";

// BOLT11 by hand, for signet invoices whose expiry a test chooses. The payee
// key rides in an `n` field, so the signature is checked against it and
// needs no recovery id.
const BECH32 = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

function bech32Polymod(values) {
  const gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
  let chk = 1;
  for (const v of values) {
    const top = chk >>> 25;
    chk = ((chk & 0x1ffffff) << 5) ^ v;
    for (let i = 0; i < 5; i++) if ((top >>> i) & 1) chk ^= gen[i];
  }
  return chk >>> 0;
}

/** Regroups bits, padding the last group with zeros. */
function regroup(values, from, to) {
  let acc = 0;
  let bits = 0;
  const out = [];
  for (const v of values) {
    acc = (acc << from) | v;
    bits += from;
    while (bits >= to) {
      bits -= to;
      out.push((acc >> bits) & ((1 << to) - 1));
    }
    acc &= (1 << bits) - 1;
  }
  if (bits > 0) out.push((acc << (to - bits)) & ((1 << to) - 1));
  return out;
}

function intWords(n, length) {
  const out = [];
  for (let i = 0; i < length || n > 0; i++) {
    out.unshift(n % 32);
    n = Math.floor(n / 32);
    if (length === undefined && n === 0) break;
  }
  return out;
}

function taggedField(tag, words) {
  return [BECH32.indexOf(tag), words.length >> 5, words.length & 31, ...words];
}

/** A signet invoice for 50 000 sat that expires `expiresIn` seconds from now. */
function signetInvoice(expiresIn = 3600) {
  const issued = Math.floor(Date.now() / 1000) + expiresIn - 3600;
  const { privateKey, publicKey } = generateKeyPairSync("ec", {
    namedCurve: "secp256k1",
  });
  const jwk = publicKey.export({ format: "jwk" });
  const y = Buffer.from(jwk.y, "base64url");
  const payee = Buffer.concat([
    Buffer.from([y[31] & 1 ? 3 : 2]),
    Buffer.from(jwk.x, "base64url"),
  ]);
  const hrp = "lntbs500u";
  const data = [
    ...intWords(issued, 7),
    ...taggedField(
      "p",
      regroup(createHash("sha256").update(String(issued)).digest(), 8, 5),
    ),
    ...taggedField("s", regroup(Buffer.alloc(32, 7), 8, 5)),
    ...taggedField("d", regroup(Buffer.from("test"), 8, 5)),
    ...taggedField("n", regroup(payee, 8, 5)),
    ...taggedField("x", intWords(3600)),
    ...taggedField("c", intWords(144)),
    // var_onion_optin and payment_secret, both required.
    ...taggedField("9", intWords(2 ** 8 + 2 ** 14)),
  ];
  const der = sign(
    "sha256",
    Buffer.concat([Buffer.from(hrp), Buffer.from(regroup(data, 5, 8))]),
    { key: privateKey, dsaEncoding: "ieee-p1363" },
  );
  // secp256k1 verification takes low-S signatures only.
  const order =
    0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
  let s = BigInt(`0x${der.subarray(32).toString("hex")}`);
  if (s > order / 2n) s = order - s;
  const signature = Buffer.concat([
    der.subarray(0, 32),
    Buffer.from(s.toString(16).padStart(64, "0"), "hex"),
    Buffer.from([0]),
  ]);
  const words = [...data, ...regroup(signature, 8, 5)];
  const expanded = [
    ...[...hrp].map((c) => c.charCodeAt(0) >> 5),
    0,
    ...[...hrp].map((c) => c.charCodeAt(0) & 31),
  ];
  const chk = bech32Polymod([...expanded, ...words, 0, 0, 0, 0, 0, 0]) ^ 1;
  const checksum = [0, 1, 2, 3, 4, 5].map((i) => (chk >>> (5 * (5 - i))) & 31);
  return `${hrp}1${[...words, ...checksum].map((w) => BECH32[w]).join("")}`;
}

const REVERSE_CARD = {
  hash: "rev-hash",
  rate: 1,
  limits: { minimal: 10000, maximal: 1000000 },
  fees: { percentage: 0.5, minerFees: { lockup: 200, claim: 100 } },
};
const SUBMARINE_CARD = {
  hash: "sub-hash",
  rate: 1,
  limits: { minimal: 10000, maximal: 1000000, maximalZeroConf: 0 },
  fees: { percentage: 0.1, minerFees: 150 },
};

/** The maker's pair catalogues, served through the global `fetch` the wasm HTTP stack uses. */
function stubMaker(t) {
  const real = globalThis.fetch;
  const requests = [];
  globalThis.fetch = async (input, init) => {
    const url = String(input?.url ?? input);
    requests.push({ url, method: init?.method ?? input?.method ?? "GET" });
    if (url.endsWith("/v2/swap/reverse"))
      return Response.json({
        BTC: { "L-BTC": REVERSE_CARD, BTC: REVERSE_CARD },
      });
    if (url.endsWith("/v2/swap/submarine"))
      return Response.json({
        BTC: { BTC: SUBMARINE_CARD },
        "L-BTC": { BTC: SUBMARINE_CARD },
      });
    return new Response("not found", { status: 404 });
  };
  t.after(() => {
    globalThis.fetch = real;
  });
  return requests;
}

const SIGNER = mnemonicSigner(
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
  "signet",
);

async function kaleido() {
  return createKaleidoSwap({
    network: "signet",
    store: new MemoryStore(),
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
    pollIntervalMs: 1,
  });
}

test("quote: reverse prices from the pair card and binds its hash", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  const answer = await k.quote({
    pay: "lightning:BTC",
    receive: "liquid:BTC",
    amount: { pay: 50_000 },
  });
  assert.ok(isQuote(answer));
  assert.equal(answer.protocol, "boltz-reverse");
  assert.equal(answer.fromAmount, 50_000n);
  assert.deepEqual(answer.fees, { maker: 250n, miner: 300n });
  assert.equal(answer.toAmount, 49_450n);
  assert.equal(answer.binding.pairHash, "rev-hash");
});

test("quote: amounts in any accepted form, bad ones coded", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  for (const amount of [50_000n, 50_000, "50000"]) {
    const answer = await k.quote({
      pay: "lightning:BTC",
      receive: "liquid:BTC",
      amount: { pay: amount },
    });
    assert.ok(isQuote(answer) && answer.fromAmount === 50_000n);
  }
  for (const amount of [1.5, "12abc", Number.MAX_SAFE_INTEGER + 1, -1]) {
    await assert.rejects(
      k.quote({
        pay: "lightning:BTC",
        receive: "liquid:BTC",
        amount: { pay: amount },
      }),
      (e) => isSwapError(e) && e.code === "INVALID_AMOUNT",
    );
  }
});

test("quote: refusals are values, not throws", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  const below = await k.quote({
    pay: "lightning:BTC",
    receive: "liquid:BTC",
    amount: { pay: 1 },
  });
  assert.equal(below.kind, "refusal");
  assert.equal(below.reason, "amount_below_min");
  assert.deepEqual(below.limits, { min: 10_000n, max: 1_000_000n });
  const arkade = await k.quote({
    pay: "arkade:BTC",
    receive: "lightning:BTC",
    amount: { pay: 1 },
  });
  assert.equal(arkade.reason, "pair_unavailable");
  const usdt = await k.quote({
    pay: "lightning:BTC",
    receive: "liquid:USDT",
    amount: { pay: 1 },
  });
  assert.equal(usdt.reason, "pair_unavailable");
  for (const [pay, receive, says] of [
    ["lightning:BTC", "bitcoin:rgb:2dkSTbr-jFhznbPmo", "RGB swap"],
    ["spark:BTC", "liquid:BTC", "Spark"],
    ["bitcoin:BTC", "liquid:BTC", "chain swap"],
  ]) {
    const refusal = await k.quote({ pay, receive, amount: { pay: 50_000 } });
    assert.equal(refusal.kind, "refusal");
    assert.equal(refusal.from, pay);
    assert.ok(refusal.message.includes(says), refusal.message);
  }
});

test("quote: a submarine is priced against the invoice and checks its network", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  const answer = await k.quote({
    pay: "bitcoin:BTC",
    receive: "lightning:BTC",
    amount: { invoice: signetInvoice() },
  });
  assert.ok(isQuote(answer));
  assert.equal(answer.protocol, "boltz-submarine");
  assert.equal(answer.toAmount, 50_000n);
  assert.equal(answer.fromAmount, 50_200n);
  await assert.rejects(
    k.quote({
      pay: "bitcoin:BTC",
      receive: "lightning:BTC",
      amount: { invoice: INVOICE_REGTEST_100K },
    }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  await assert.rejects(
    k.quote({
      pay: "bitcoin:BTC",
      receive: "lightning:BTC",
      amount: { invoice: signetInvoice(-60) },
    }),
    (e) => isSwapError(e) && e.code === "INVOICE_EXPIRED",
  );
  await assert.rejects(
    k.quote({
      pay: "bitcoin:BTC",
      receive: "lightning:BTC",
      amount: { pay: 50_000 },
    }),
    (e) => e.code === "INVALID_DESTINATION",
  );
  await assert.rejects(
    k.quote({ pay: "bitcoin:BTC", receive: "lightning:BTC" }),
    (e) => isSwapError(e) && e.code === "INVALID_AMOUNT",
  );
});

test("start: destination and refund address are checked before the maker is contacted", async (t) => {
  const requests = stubMaker(t);
  const k = await kaleido();
  const reverse = await k.quote({
    pay: "lightning:BTC",
    receive: "liquid:BTC",
    amount: { pay: 50_000 },
  });
  await assert.rejects(
    k.start(reverse, {
      destination: "ex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j20sf6s5",
    }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  await assert.rejects(
    k.start(reverse, {}),
    (e) => e.code === "INVALID_DESTINATION",
  );
  await assert.rejects(
    k.start(
      { ...reverse, expiresAt: 0n },
      { destination: "tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl" },
    ),
    (e) => e.code === "QUOTE_EXPIRED",
  );

  const submarine = await k.quote({
    pay: "bitcoin:BTC",
    receive: "lightning:BTC",
    amount: { invoice: signetInvoice() },
  });
  await assert.rejects(
    k.start(submarine, {}),
    (e) => e.code === "REFUND_ADDRESS_REQUIRED",
  );
  await assert.rejects(
    k.start(submarine, {
      refundAddress: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
    }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  assert.equal(
    requests.filter((r) => r.method === "POST").length,
    0,
    "nothing was created",
  );
});

test("options: reverseTimeoutHeadroom reaches the core and its keys are checked", async () => {
  const options = {
    network: "signet",
    store: new MemoryStore(),
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
  };
  const k = await createKaleidoSwap({
    ...options,
    reverseTimeoutHeadroom: { bitcoinBlocks: 20, liquidBlocks: 200 },
  });
  assert.equal(k.network, "signet");
  // A misspelled chain is refused rather than silently left at the default.
  await assert.rejects(
    createKaleidoSwap({ ...options, reverseTimeoutHeadroom: { liquid: 200 } }),
  );
});

test("options: feePolicy reaches the core and its keys and values are checked", async () => {
  const options = {
    network: "signet",
    store: new MemoryStore(),
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
  };
  const k = await createKaleidoSwap({
    ...options,
    feePolicy: { targetBlocks: 3, bitcoinMaxSatPerVb: 100, maxFeePercent: 5 },
  });
  assert.equal(k.network, "signet");
  await assert.rejects(
    createKaleidoSwap({ ...options, feePolicy: { target: 3 } }),
  );
  await assert.rejects(
    createKaleidoSwap({
      ...options,
      feePolicy: { bitcoinMinSatPerVb: 10, bitcoinMaxSatPerVb: 5 },
    }),
    (e) => e.code === "INVALID_AMOUNT",
  );
  await assert.rejects(
    createKaleidoSwap({ ...options, feeSatPerVb: 0 }),
    (e) => e.code === "INVALID_AMOUNT",
  );
});

test("swap() and resume() on an empty store", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  await assert.rejects(k.swap("nope"), (e) => e.code === "SWAP_NOT_FOUND");
  assert.deepEqual(await k.resume(), {
    resumed: [],
    settled: [],
    refunded: [],
    failed: [],
    errors: [],
  });
});

/** A reverse-swap record as the store holds it, as written by the Rust core. */
function seededRecord(overrides = {}) {
  return {
    id: "seed-1",
    protocol: "boltz-reverse",
    network: "signet",
    makerUrl: "https://maker.example/v2",
    from: "lightning:BTC",
    to: "liquid:BTC",
    phase: "awaiting_funding",
    rawStatus: "swap.created",
    createdAt: 1_700_000_000,
    updatedAt: 1_700_000_000,
    state: {
      swapIndex: 0,
      response: {
        protocol: "boltz-reverse",
        id: "seed-1",
        invoice: INVOICE_SIGNET_50K,
        swapTree: {
          claimLeaf: { output: "00", version: 196 },
          refundLeaf: { output: "01", version: 196 },
        },
        lockupAddress: "tlq1lockup",
        refundPublicKey:
          "0276177bcce18ee504d87511991653ca9736a32f58066331e8bc93f1a3cf5dd1f2",
        timeoutBlockHeight: 200,
        onchainAmount: 49_450,
        swapAuth: "secret-auth",
      },
      destination: "tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl",
    },
    ...overrides,
  };
}

test("records cross the wasm boundary as plain objects the store can serialize", async (t) => {
  const real = globalThis.fetch;
  globalThis.fetch = async (input) => {
    const url = String(input?.url ?? input);
    if (url.endsWith("/v2/swap/seed-1"))
      return Response.json({ status: "transaction.mempool" });
    return new Response("not found", { status: 404 });
  };
  t.after(() => {
    globalThis.fetch = real;
  });

  const written = [];
  const store = {
    records: [seededRecord()],
    get: () => undefined,
    put(record) {
      written.push(record);
    },
    list() {
      return this.records;
    },
    nextIndex: () => 1n,
  };
  const k = await createKaleidoSwap({
    network: "signet",
    store,
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
    pollIntervalMs: 1,
  });
  const report = await k.resume();

  assert.equal(report.resumed.length, 1);
  assert.equal(written.length, 1, "the observed phase was persisted");
  const [record] = written;
  assert.equal(record.constructor, Object, "a plain object, not a Map");
  assert.equal(record.id, "seed-1");
  assert.equal(record.phase, "funded");
  assert.equal(record.rawStatus, "transaction.mempool");
  assert.equal(record.state.response.swapAuth, "secret-auth");
  assert.equal(record.state.response.onchainAmount, 49_450);
  // JSON.stringify must write the record whole: a Map becomes "{}", a bigint throws.
  assert.deepEqual(
    JSON.parse(JSON.stringify(record)).state.response.id,
    "seed-1",
  );
  assert.equal(report.resumed[0].record().id, "seed-1");
  assert.equal(report.resumed[0].phase, "funded");
});

test("settled() with an already aborted signal rejects ABORTED and leaves no unhandled rejection", async (t) => {
  const real = globalThis.fetch;
  let status = "transaction.mempool";
  globalThis.fetch = async (input) => {
    const url = String(input?.url ?? input);
    if (url.endsWith("/v2/swap/seed-1")) return Response.json({ status });
    return new Response("not found", { status: 404 });
  };
  const unhandled = [];
  const onUnhandled = (reason) => unhandled.push(reason);
  process.on("unhandledRejection", onUnhandled);
  t.after(() => {
    globalThis.fetch = real;
    process.off("unhandledRejection", onUnhandled);
  });

  const store = {
    records: [seededRecord()],
    get: () => undefined,
    put() {},
    list() {
      return this.records;
    },
    nextIndex: () => 1n,
  };
  const k = await createKaleidoSwap({
    network: "signet",
    store,
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
    pollIntervalMs: 1,
  });
  const { resumed } = await k.resume();
  status = "swap.expired"; // the swap now ends in `failed`, so the inner settled() rejects
  await assert.rejects(
    resumed[0].settled(AbortSignal.abort()),
    (e) => e.code === "ABORTED",
  );
  await new Promise((resolve) => setTimeout(resolve, 200));
  assert.deepEqual(unhandled, []);
});

test("FileStore: concurrent writes of one record never leave a torn file", async () => {
  const dir = await mkdtemp(join(tmpdir(), "kaleidoswap-store-"));
  try {
    const store = new FileStore(dir);
    const record = seededRecord();
    await Promise.all(
      Array.from({ length: 64 }, (_, i) =>
        store.put({ ...record, rawStatus: `status-${i}` }),
      ),
    );
    const back = await store.get("seed-1");
    assert.equal(back.id, "seed-1");
    assert.match(back.rawStatus, /^status-\d+$/);
    const names = await readdir(dir);
    assert.deepEqual(
      names.filter((n) => n.endsWith(".tmp")),
      [],
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("start: an index that is not a safe integer is refused, not rounded", async (t) => {
  stubMaker(t);
  const store = new MemoryStore();
  store.nextIndex = () => Number.MAX_SAFE_INTEGER + 2;
  const k = await createKaleidoSwap({
    network: "signet",
    store,
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
    pollIntervalMs: 1,
  });
  const reverse = await k.quote({
    pay: "lightning:BTC",
    receive: "liquid:BTC",
    amount: { pay: 50_000 },
  });
  await assert.rejects(
    k.start(reverse, {
      destination: "tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl",
    }),
    (e) => e.code === "STORE_FAILED",
  );
});

test("phases(): abandoning the iterator leaves no unhandled rejection", async (t) => {
  const real = globalThis.fetch;
  let status = "swap.created";
  globalThis.fetch = async (input) => {
    const url = String(input?.url ?? input);
    if (url.endsWith("/v2/swap/seed-1")) return Response.json({ status });
    return new Response("not found", { status: 404 });
  };
  const unhandled = [];
  const onUnhandled = (reason) => unhandled.push(reason);
  process.on("unhandledRejection", onUnhandled);
  t.after(() => {
    globalThis.fetch = real;
    process.off("unhandledRejection", onUnhandled);
  });
  const store = {
    records: [seededRecord()],
    get: () => undefined,
    put() {},
    list() {
      return this.records;
    },
    nextIndex: () => 1n,
  };
  const k = await createKaleidoSwap({
    network: "signet",
    store,
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
    pollIntervalMs: 1,
  });
  const { resumed } = await k.resume();
  status = "transaction.mempool"; // the first transition the iterator sees
  for await (const phase of resumed[0].phases()) {
    assert.equal(phase, "funded");
    break; // the consumer walks away…
  }
  status = "swap.expired"; // …and the swap then ends in `failed`
  await new Promise((resolve) => setTimeout(resolve, 200));
  assert.deepEqual(unhandled, []);
});

test("a store that throws surfaces as STORE_FAILED", async (t) => {
  stubMaker(t);
  const failing = {
    get() {
      throw new Error("disk gone");
    },
    put() {
      throw new Error("disk gone");
    },
    list() {
      throw new Error("disk gone");
    },
    nextIndex() {
      throw new Error("disk gone");
    },
  };
  const k = await createKaleidoSwap({
    network: "signet",
    store: failing,
    signer: SIGNER,
    makerUrl: "https://maker.example/v2",
  });
  await assert.rejects(k.resume(), (e) => e.code === "STORE_FAILED");
});

test("FileStore: records round-trip as JSON and the index never repeats", async () => {
  const dir = await mkdtemp(join(tmpdir(), "kaleidoswap-store-"));
  try {
    const store = new FileStore(dir);
    assert.deepEqual(await store.list(), []);
    assert.equal(await store.nextIndex(), 0n);
    const [a, b] = await Promise.all([store.nextIndex(), store.nextIndex()]);
    assert.deepEqual([a, b].sort(), [1n, 2n]);
    assert.equal((await readFile(join(dir, "index"), "utf8")).trim(), "3");

    const record = {
      id: "x",
      protocol: "boltz-reverse",
      network: "signet",
      makerUrl: "https://maker.example/v2",
      from: "lightning:BTC",
      to: "liquid:BTC",
      phase: "awaiting_funding",
      rawStatus: "swap.created",
      createdAt: "now",
      updatedAt: "now",
      state: {
        swapIndex: "0",
        response: { onchainAmount: "1" },
        destination: "tex1q50rtrmj2f8vl9tem8qpfw36ylw5jg9j24kmnvl",
      },
    };
    await store.put(record);
    assert.deepEqual(await store.get("x"), record);
    assert.deepEqual(await store.list(), [record]);
    assert.equal(await store.get("missing"), undefined);

    // A second instance on the same directory continues the counter.
    assert.equal(await new FileStore(dir).nextIndex(), 3n);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("mnemonicSigner derives what SwapMasterKey derives", async () => {
  await init();
  const mnemonic =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
  const master = SwapMasterKey.fromSwapMnemonic(mnemonic, "signet");
  const signer = mnemonicSigner(mnemonic, "signet");
  assert.equal(await signer.publicKey(7n), master.deriveSwapKey(7n).publicKey);
  assert.deepEqual(await signer.derivePreimage(7n), master.derivePreimage(7n));
  assert.match(
    await signer.signSchnorr(7n, "11".repeat(32)),
    /^[0-9a-f]{128}$/,
  );
});

test("mnemonicSigner keeps its MuSig2 nonce and signs with it once", async () => {
  await init();
  const ours = mnemonicSigner(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "signet",
  );
  const theirs = mnemonicSigner(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "signet",
    "maker",
  );
  const session = {
    pubkeys: [await theirs.publicKey(0), await ours.publicKey(0)],
    tweak: "03".repeat(32),
    sighash: "05".repeat(32),
  };
  const ourNonce = await ours.musigNonce(0, session);
  const theirNonce = await theirs.musigNonce(0, session);
  assert.match(ourNonce, /^[0-9a-f]{132}$/);
  const partial = await ours.musigPartialSign(0, session, ourNonce, theirNonce);
  assert.match(partial, /^[0-9a-f]{64}$/);
  assert.throws(() => ours.musigPartialSign(0, session, ourNonce, theirNonce));
});

test("FileStore: a swap id never names a path outside the directory", async () => {
  const dir = await mkdtemp(join(tmpdir(), "kaleidoswap-store-"));
  try {
    const store = new FileStore(dir);
    for (const id of ["../escape", "a/b", ".hidden", "", "x".repeat(200)]) {
      await assert.rejects(store.get(id), (e) => e.code === "STORE_FAILED");
      await assert.rejects(store.put({ id }), (e) => e.code === "STORE_FAILED");
    }
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
