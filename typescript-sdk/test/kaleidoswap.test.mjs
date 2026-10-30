// Tests for the high-level surface on the TypeScript side. The phase machine
// itself — pricing, validation, persistence order, claim and refund, resume —
// is the Rust core's and is tested there against a fake maker
// (`src/swaps/kaleidoswap.rs`). Here: the wasm binding end to end for the
// paths that need no valid swap tree (quote, and the checks `start` makes
// before it contacts the maker), the adapters, and the Node stores.
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
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

// A hold invoice the signet maker issued (50 000 sat), and a regtest one.
const INVOICE_SIGNET_50K =
  "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e";
const INVOICE_REGTEST_100K =
  "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysyt5fzeueg687mkcum4we2pm452clxqdy9l4extcqs84ek4nte3s56s6pmwxhmx4zx3d4k6xsftlphw8ed60xerfchczpw8w8avdn92qpftvz36";

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
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
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
      from: "lightning:BTC",
      to: "liquid:BTC",
      amount,
    });
    assert.ok(isQuote(answer) && answer.fromAmount === 50_000n);
  }
  for (const amount of [1.5, "12abc", Number.MAX_SAFE_INTEGER + 1, -1]) {
    await assert.rejects(
      k.quote({ from: "lightning:BTC", to: "liquid:BTC", amount }),
      (e) => isSwapError(e) && e.code === "INVALID_AMOUNT",
    );
  }
});

test("quote: refusals are values, not throws", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  const below = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 1,
  });
  assert.equal(below.kind, "refusal");
  assert.equal(below.reason, "amount_below_min");
  assert.deepEqual(below.limits, { min: 10_000n, max: 1_000_000n });
  const arkade = await k.quote({
    from: "arkade:BTC",
    to: "lightning:BTC",
    amount: 1,
  });
  assert.equal(arkade.reason, "pair_unavailable");
  const usdt = await k.quote({
    from: "lightning:BTC",
    to: "liquid:USDT",
    amount: 1,
  });
  assert.equal(usdt.reason, "pair_unavailable");
});

test("quote: a submarine is priced against the invoice and checks its network", async (t) => {
  stubMaker(t);
  const k = await kaleido();
  const answer = await k.quote({
    from: "bitcoin:BTC",
    to: "lightning:BTC",
    invoice: INVOICE_SIGNET_50K,
  });
  assert.ok(isQuote(answer));
  assert.equal(answer.protocol, "boltz-submarine");
  assert.equal(answer.toAmount, 50_000n);
  assert.equal(answer.fromAmount, 50_200n);
  await assert.rejects(
    k.quote({
      from: "bitcoin:BTC",
      to: "lightning:BTC",
      invoice: INVOICE_REGTEST_100K,
    }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  await assert.rejects(
    k.quote({ from: "bitcoin:BTC", to: "lightning:BTC" }),
    (e) => e.code === "INVALID_DESTINATION",
  );
});

test("start: destination and refund address are checked before the maker is contacted", async (t) => {
  const requests = stubMaker(t);
  const k = await kaleido();
  const reverse = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  await assert.rejects(
    k.start(reverse, { destination: "lq1mainnet" }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  await assert.rejects(
    k.start(reverse, {}),
    (e) => e.code === "INVALID_DESTINATION",
  );
  await assert.rejects(
    k.start({ ...reverse, expiresAt: 0n }, { destination: "tlq1dest" }),
    (e) => e.code === "QUOTE_EXPIRED",
  );

  const submarine = await k.quote({
    from: "bitcoin:BTC",
    to: "lightning:BTC",
    invoice: INVOICE_SIGNET_50K,
  });
  await assert.rejects(
    k.start(submarine, {}),
    (e) => e.code === "REFUND_ADDRESS_REQUIRED",
  );
  await assert.rejects(
    k.start(submarine, { refundAddress: "bc1mainnet" }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  assert.equal(
    requests.filter((r) => r.method === "POST").length,
    0,
    "nothing was created",
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
      destination: "tlq1dest",
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
        destination: "tlq1",
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
  assert.deepEqual(await signer.deriveKey(7n), master.deriveSwapKey(7n));
  assert.deepEqual(await signer.derivePreimage(7n), master.derivePreimage(7n));
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
