// Tests for the high-level surface. The maker is a fake `SwapClient` fed a
// status script; the chain steps go through the `flows` seam, so nothing here
// touches a network or builds a transaction. The one wasm-backed test at the
// end checks that `mnemonicSigner` derives what `SwapMasterKey` does.
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  MemoryStore,
  isQuote,
  isSwapError,
  makeCreateKaleidoSwap,
} from "../dist/kaleidoswap.js";
import {
  FileStore,
  SwapMasterKey,
  init,
  mnemonicSigner,
} from "../dist/index.node.js";

// Hold invoices the signet maker issued (50 000 sat) and a regtest one, from
// pay-through.test.mjs. Only amount and prefix matter here.
const INVOICE_SIGNET_50K =
  "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e";
const INVOICE_REGTEST_100K =
  "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysyt5fzeueg687mkcum4we2pm452clxqdy9l4extcqs84ek4nte3s56s6pmwxhmx4zx3d4k6xsftlphw8ed60xerfchczpw8w8avdn92qpftvz36";

const REVERSE_CARD = {
  hash: "rev-hash",
  rate: 1,
  limits: { minimal: 10_000n, maximal: 1_000_000n },
  fees: { percentage: 0.5, minerFees: { lockup: 200n, claim: 100n } },
};
const SUBMARINE_CARD = {
  hash: "sub-hash",
  rate: 1,
  limits: { minimal: 10_000n, maximal: 1_000_000n, maximalZeroConf: 0n },
  fees: { percentage: 0.1, minerFees: 150n },
};

/** A maker that answers from a script of statuses, one per `swap()` call. */
class FakeClient {
  constructor(url) {
    this.url = url;
    this.statuses = [];
    this.created = [];
  }
  async reversePairs() {
    return { BTC: { "L-BTC": REVERSE_CARD, BTC: REVERSE_CARD } };
  }
  async submarinePairs() {
    return { BTC: { BTC: SUBMARINE_CARD }, "L-BTC": { BTC: SUBMARINE_CARD } };
  }
  async createReverseSwap(network, req) {
    this.created.push({ kind: "reverse", network, req });
    return {
      id: "rev-1",
      invoice: INVOICE_SIGNET_50K,
      lockupAddress: "tlq1lockup",
      onchainAmount: req.invoiceAmount - 550n,
      swapAuth: "secret-auth",
      timeoutBlockHeight: 100n,
    };
  }
  async createSubmarineSwap(network, req) {
    this.created.push({ kind: "submarine", network, req });
    return {
      id: "sub-1",
      address: "tb1qlockup",
      expectedAmount: 50_200n,
      acceptZeroConf: false,
      swapAuth: "secret-auth",
      timeoutBlockHeight: 100n,
    };
  }
  async swap() {
    if (this.statuses.length === 0) throw new Error("script exhausted");
    const next =
      this.statuses.length > 1 ? this.statuses.shift() : this.statuses[0];
    return { status: next };
  }
}

function harness({
  network = "signet",
  store = new MemoryStore(),
  flows,
} = {}) {
  let client;
  const calls = { verify: [], claim: [], refund: [] };
  const create = makeCreateKaleidoSwap({
    init: async () => {},
    SwapClient: class extends FakeClient {
      constructor(url) {
        super(url);
        client = this;
      }
      static forNetwork() {
        throw new Error("tests always pass makerUrl");
      }
    },
    SwapScript: {},
  });
  const signer = {
    async deriveKey(index) {
      return { publicKey: `pub-${index}`, secretKey: `sec-${index}` };
    },
    async derivePreimage(index) {
      return { preimage: `pre-${index}`, sha256: `hash-${index}`, hash160: "" };
    },
  };
  const defaultFlows = {
    verifyLockup(record, pubkey) {
      calls.verify.push([record.id, pubkey]);
    },
    async claim(record, key, preimage) {
      calls.claim.push([record.id, key.publicKey, preimage]);
      return "claim-txid";
    },
    async refund(record, key) {
      calls.refund.push([record.id, key.publicKey]);
      return "refund-txid";
    },
  };
  const kaleido = create(
    {
      network,
      store,
      signer,
      makerUrl: "https://maker.example/v2",
      pollIntervalMs: 1,
    },
    { ...defaultFlows, ...flows },
  );
  return { kaleido, store, calls, client: () => client };
}

test("quote: reverse prices from the pair card and binds its hash", async () => {
  const { kaleido } = harness();
  const answer = await (
    await kaleido
  ).quote({
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

test("quote: side 'to' inverts the fee formula", async () => {
  const { kaleido } = harness();
  const answer = await (
    await kaleido
  ).quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 49_450n,
    side: "to",
  });
  assert.ok(isQuote(answer));
  assert.ok(answer.toAmount >= 49_450n, `${answer.toAmount}`);
  assert.ok(answer.fromAmount - 50_000n <= 1n, `${answer.fromAmount}`);
});

test("quote: refusals are values, not throws", async () => {
  const k = await harness().kaleido;
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
  assert.match(arkade.message, /arkade/);

  const usdt = await k.quote({
    from: "lightning:BTC",
    to: "liquid:USDT",
    amount: 1,
  });
  assert.equal(usdt.reason, "pair_unavailable");
});

test("quote: bad amounts are coded errors", async () => {
  const k = await harness().kaleido;
  for (const amount of [1.5, "12abc", Number.MAX_SAFE_INTEGER + 1]) {
    await assert.rejects(
      k.quote({ from: "lightning:BTC", to: "liquid:BTC", amount }),
      (e) => isSwapError(e) && e.code === "INVALID_AMOUNT",
    );
  }
});

test("quote: submarine is priced against the invoice and checks its network", async () => {
  const k = await harness().kaleido;
  const answer = await k.quote({
    from: "bitcoin:BTC",
    to: "lightning:BTC",
    invoice: INVOICE_SIGNET_50K,
  });
  assert.ok(isQuote(answer));
  assert.equal(answer.protocol, "boltz-submarine");
  assert.equal(answer.toAmount, 50_000n);
  assert.equal(answer.fromAmount, 50_000n + 50n + 150n);

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
  await assert.rejects(
    k.quote({
      from: "bitcoin:BTC",
      to: "lightning:BTC",
      invoice: INVOICE_SIGNET_50K,
      amount: 1,
    }),
    (e) => e.code === "INVALID_AMOUNT",
  );
});

test("start: validates the destination, verifies the lockup, persists before returning", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });

  await assert.rejects(
    k.start(quote, { destination: "lq1mainnet" }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );
  await assert.rejects(
    k.start(quote, {}),
    (e) => e.code === "INVALID_DESTINATION",
  );
  assert.equal(h.client().created.length, 0, "nothing reached the maker");

  const swap = await k.start(quote, { destination: "tlq1dest" });
  assert.equal(swap.id, "rev-1");
  assert.equal(swap.phase, "awaiting_funding");
  assert.equal(swap.action.invoice, INVOICE_SIGNET_50K);
  assert.deepEqual(h.calls.verify, [["rev-1", "pub-0"]]);

  const { req } = h.client().created[0];
  assert.deepEqual(req, {
    from: "BTC",
    to: "L-BTC",
    invoiceAmount: 50_000n,
    preimageHash: "hash-0",
    claimPublicKey: "pub-0",
    pairHash: "rev-hash",
  });

  const record = await h.store.get("rev-1");
  assert.equal(record.phase, "awaiting_funding");
  assert.equal(record.state.swapIndex, "0");
  assert.equal(record.state.response.swapAuth, "secret-auth");
  // JSON-plain: the amount crossed as bigint and is stored as a string.
  assert.equal(record.state.response.onchainAmount, "49450");
  assert.deepEqual(record.state.bigints, [
    "onchainAmount",
    "timeoutBlockHeight",
  ]);
  assert.doesNotThrow(
    () => JSON.stringify(record),
    "no bigint left in the record",
  );
});

test("start: a lockup that does not match our key never reaches the store", async () => {
  const h = harness({
    flows: {
      verifyLockup() {
        throw new Error("tree mismatch");
      },
    },
  });
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  await assert.rejects(
    k.start(quote, { destination: "tlq1dest" }),
    (e) => e.code === "LOCKUP_MISMATCH",
  );
  assert.equal(await h.store.get("rev-1"), undefined);
});

test("start: each swap takes a fresh index; expired quotes are refused", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  await k.start(quote, { destination: "tlq1a" });
  await k.start(quote, { destination: "tlq1b" });
  assert.deepEqual(
    h.client().created.map((c) => c.req.claimPublicKey),
    ["pub-0", "pub-1"],
  );
  await assert.rejects(
    k.start({ ...quote, expiresAt: new Date(0) }, { destination: "tlq1a" }),
    (e) => e.code === "QUOTE_EXPIRED",
  );
});

test("reverse: phases run to settled and the claim fires once, on confirmation", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  const swap = await k.start(quote, { destination: "tlq1dest" });
  h.client().statuses = [
    "swap.created",
    "transaction.mempool",
    "transaction.confirmed",
    "transaction.confirmed", // still reported after our claim broadcast
    "invoice.settled",
  ];

  const seen = [];
  for await (const phase of swap.phases()) seen.push(phase);
  assert.deepEqual(seen, ["funded", "locked", "claiming", "settled"]);
  assert.deepEqual(h.calls.claim, [["rev-1", "pub-0", "pre-0"]]);

  const record = await h.store.get("rev-1");
  assert.equal(record.phase, "settled");
  assert.equal(record.state.claimTxid, "claim-txid");
  assert.equal(record.rawStatus, "invoice.settled");
});

test("reverse: settled() rejects with the terminal reason", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  const swap = await k.start(quote, { destination: "tlq1dest" });
  h.client().statuses = ["swap.created", "swap.expired"];
  await assert.rejects(
    swap.settled(),
    (e) => e.code === "SWAP_EXPIRED" && e.swapId === "rev-1",
  );
  assert.equal(
    h.calls.refund.length,
    0,
    "a reverse swap has nothing of ours to refund",
  );
});

test("submarine: funding then a failed payment refunds on its own", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "bitcoin:BTC",
    to: "lightning:BTC",
    invoice: INVOICE_SIGNET_50K,
  });

  await assert.rejects(
    k.start(quote, {}),
    (e) => e.code === "REFUND_ADDRESS_REQUIRED",
  );
  await assert.rejects(
    k.start(quote, { refundAddress: "bc1mainnet" }),
    (e) => e.code === "WRONG_NETWORK_ADDRESS",
  );

  const swap = await k.start(quote, { refundAddress: "tb1qrefund" });
  assert.deepEqual(swap.action, {
    fundingAddress: "tb1qlockup",
    fundingAmount: 50_200n,
    zeroConfLimit: 0n,
  });
  assert.equal(h.client().created[0].req.invoice, INVOICE_SIGNET_50K);

  h.client().statuses = [
    "transaction.mempool",
    "invoice.failedToPay",
    "invoice.failedToPay",
    "transaction.refunded",
  ];
  const seen = [];
  for await (const phase of swap.phases()) seen.push(phase);
  assert.deepEqual(seen, ["funded", "refundable", "refunding", "refunded"]);
  assert.deepEqual(h.calls.refund, [["sub-1", "pub-0"]]);
});

test("submarine: expiry before funding is failed, not refundable", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "bitcoin:BTC",
    to: "lightning:BTC",
    invoice: INVOICE_SIGNET_50K,
  });
  const swap = await k.start(quote, { refundAddress: "tb1qrefund" });
  h.client().statuses = ["swap.created", "swap.expired"];
  const seen = [];
  for await (const phase of swap.phases()) seen.push(phase);
  assert.deepEqual(seen, ["failed"]);
  assert.equal(h.calls.refund.length, 0);
});

test("resume: one pass over the store acts on what is actionable and reports the rest", async () => {
  const store = new MemoryStore();
  const first = harness({ store });
  const k1 = await first.kaleido;
  const quote = await k1.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  await k1.start(quote, { destination: "tlq1dest" });

  // A new process over the same store: the previous one never saw the lockup.
  const second = harness({ store });
  const k2 = await second.kaleido;
  second.client().statuses = ["transaction.confirmed"];
  const report = await k2.resume();
  assert.equal(report.resumed.length, 1);
  assert.equal(report.resumed[0].phase, "claiming");
  assert.deepEqual(second.calls.claim, [["rev-1", "pub-0", "pre-0"]]);
  assert.equal((await store.get("rev-1")).state.claimTxid, "claim-txid");

  // A third pass sees the claim already broadcast and does not repeat it.
  const third = harness({ store });
  const k3 = await third.kaleido;
  third.client().statuses = ["transaction.confirmed", "invoice.settled"];
  const again = await k3.resume();
  assert.equal(third.calls.claim.length, 0);
  assert.equal(again.resumed.length, 1);
  assert.equal((await k3.swap("rev-1")).phase, "claiming");
  const done = await again.resumed[0].settled();
  assert.equal(done.phase, "settled");

  const settled = await k3.resume();
  assert.deepEqual(settled, {
    resumed: [],
    settled: [],
    refunded: [],
    failed: [],
  });
});

test("swap(): unknown ids are coded", async () => {
  const k = await harness().kaleido;
  await assert.rejects(k.swap("nope"), (e) => e.code === "SWAP_NOT_FOUND");
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
      await assert.rejects(store.get(id), (e) => e.code === "SWAP_FAILED");
      await assert.rejects(store.put({ id }), (e) => e.code === "SWAP_FAILED");
    }
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("start: a maker-supplied id that is not a plain token is refused", async () => {
  const h = harness();
  const k = await h.kaleido;
  const quote = await k.quote({
    from: "lightning:BTC",
    to: "liquid:BTC",
    amount: 50_000,
  });
  const client = h.client();
  const real = client.createReverseSwap.bind(client);
  client.createReverseSwap = async (...a) => ({
    ...(await real(...a)),
    id: "../x",
  });
  await assert.rejects(
    k.start(quote, { destination: "tlq1dest" }),
    (e) => e.code === "MAKER_REJECTED",
  );
  assert.deepEqual(await h.store.list(), []);
});
