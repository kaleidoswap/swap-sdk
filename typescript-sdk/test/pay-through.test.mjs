import assert from "node:assert/strict";
import test from "node:test";
import { PayThroughApiError, PayThroughClient } from "../dist/pay-through.js";

const created = {
  id: "swap_123",
  swapAuth: "ab".repeat(32),
  invoice: "lnbcrt1test",
  paymentHash: "cd".repeat(32),
  destination: "bcrt1test",
  destinationLayer: "BTC_L1",
  payoutAsset: "BTC",
  pairId: "BTC@LN/BTC@L1",
  invoiceAmount: 100000,
  payoutAmount: 99000,
  fees: { protocol: 0, network: 500, swap: 500 },
  expiresAt: 1800000000,
};

test("pay-through create sends one amount and validates the maker's terms", async () => {
  let calls = 0;
  const client = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async (url, init) => {
      calls++;
      assert.equal(url, "https://maker.example/v2/swap/pay");
      assert.equal(init.redirect, "error");
      assert.equal(init.headers.Authorization, undefined);
      assert.deepEqual(JSON.parse(init.body), {
        destination: "bcrt1test",
        invoiceAmount: 100000,
      });
      return Response.json(created, { status: 201 });
    },
  });
  const response = await client.create({
    destination: "bcrt1test",
    invoiceAmount: 100000,
  });
  assert.deepEqual(response, created);
  await assert.rejects(
    client.create({
      destination: "bcrt1test",
      invoiceAmount: 1,
      payoutAmount: 1,
    }),
    /exactly one/,
  );
  await assert.rejects(
    client.create({
      destination: "bcrt1test",
      payoutAmount: Number.MAX_SAFE_INTEGER + 1,
    }),
    /safe integer/,
  );
  assert.equal(calls, 1);
});

test("pay-through status exposes broadcast reference and settlement state", async () => {
  const client = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async (url) => {
      assert.equal(url, "https://maker.example/v2/swap/swap_123");
      return Response.json({
        id: "swap_123",
        type: "reverse",
        status: "transaction.mempool",
        paymentStatus: "held",
        failureReason: null,
        failureDetails: null,
        events: [{ ts: 1800000000, kind: "payout_sent" }],
        payout: {
          mode: "direct",
          destination: "bcrt1test",
          layer: "BTC_L1",
          reference: "txid",
        },
      });
    },
  });
  const status = await client.status("swap_123");
  assert.equal(status.payout.reference, "txid");
  assert.equal(status.status, "transaction.mempool");
});

test("pay-through fails closed on maker errors and unsafe response amounts", async () => {
  const disabled = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async () =>
      Response.json({ error: "pay_through_disabled" }, { status: 503 }),
  });
  await assert.rejects(
    disabled.create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    (error) =>
      error instanceof PayThroughApiError &&
      error.status === 503 &&
      error.code === "pay_through_disabled",
  );
  const unsafe = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async () =>
      Response.json(
        { ...created, payoutAmount: Number.MAX_SAFE_INTEGER + 1 },
        { status: 201 },
      ),
  });
  await assert.rejects(
    unsafe.create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    /payoutAmount/,
  );
});
