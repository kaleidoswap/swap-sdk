import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import test from "node:test";
import {
  PayThroughApiError,
  PayThroughClient,
  decodeBolt11,
} from "../dist/pay-through.js";

// Generated with lightning-invoice 0.34.1: payment hash 0xcd..cd (or 0xee..ee),
// timestamp 1800000000.
const INVOICE_100K =
  "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysyt5fzeueg687mkcum4we2pm452clxqdy9l4extcqs84ek4nte3s56s6pmwxhmx4zx3d4k6xsftlphw8ed60xerfchczpw8w8avdn92qpftvz36";
const INVOICE_100001 =
  "lnbcrt1000010n1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysugtlr4n2vlv7c07cttj202gjnqge4asa73yruzj9awnmrt7gz7m36anxrfzz4wcuy3jw3rkp532a7gxt8wsnhfxy69vstq7a53fmagspg35c9h";
const INVOICE_100K_OTHER_HASH =
  "lnbcrt1m1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5amhwamhwamhwamhwamhwamhwamhwamhwamhwamhwamhwamhwamhqsp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzysc5rm78vw24gv8356vuf8mqr7ns9cxk7ns0epcs4caw7cw5zmh8uhv85g74wda59w9754j7tk28ra8dlexakz9q84qmqjq370sy5jdhcpt6mq94";
const INVOICE_NO_AMOUNT =
  "lnbcrt1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzys25qken9mn2408jgl980yn7kpmhn5cf03g2wcwdarhu0cschegp53pmenkjw5xu79gq3geu9qz04ylhvlhqtp8tk7r3q5yr5rd28hplqqs0sf7a";
const INVOICE_SIGNET_1SAT =
  "lntbs10n1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzys8v0c7343dezepc44ay9rw2c3uhrdkv7vvrle5wcuwf430dt44rfkwrhwxnnckxu3aexsxjdl006euw6lflz2znmn73x0cwqjnfunspcqxlly5m";
const INVOICE_MAINNET_25K =
  "lnbc250u1p45n5sqdq6wpshjtt5dpex7at8dqs8getnwspp5ehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxumnwdehxssp5gfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpq9qrsgqcqzyst4rvwjhcngg2sth7uqj6w8esgsdtvjmnykcm8t9l5nup6nh47ehjcff7jg36lgpm09x24ksxswgmmqp2a8wylc3ssj74upfqaqq20tcp3trzz6";
// A hold invoice the signet maker issued for a 50 000 sat reverse swap.
const INVOICE_SIGNET_MAKER =
  "lntbs500u1p4t29lqdp8ddskcetfv3hhxampwqs8yetkv4e8xefqwdmkzuqnp4qwveczq4f9q9pj47ey3wjdt4vlsk76jreecz4srms75z79feew0m7pp5d9567gdr5ay9yvy38m0lmcz0g4ny2k9m7gu02t5d06d5dxsnx9ussp526y6s8c3x3hhrgusjhtl79jh0x3x022lgywgr2llnqrrxaz3pk6s9qyysgqcqzd8xqrpc8wrms9ewjwqft6a8w585gx2pwmx8vwcm65tvkfq9sr4rp5sul37n947eyzcrdfgmxjxq4z7mwhlrtxmzw7ms8shrl0gu80l49fn7pxtsquc5q2e";

const created = {
  id: "01J9ZSWAPIDULID0000000000",
  swapAuth: "ab".repeat(32),
  invoice: INVOICE_100K,
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

function clientReturning(body, init = { status: 201 }, options = {}) {
  return new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async () => Response.json(body, init),
    ...options,
  });
}

test("decodeBolt11 reads amount and payment hash", () => {
  assert.deepEqual(decodeBolt11(INVOICE_100K), {
    amountMsat: 100_000_000n,
    paymentHash: "cd".repeat(32),
  });
  assert.equal(decodeBolt11(INVOICE_100001).amountMsat, 100_001_000n);
  assert.equal(decodeBolt11(INVOICE_SIGNET_1SAT).amountMsat, 1_000n);
  assert.equal(decodeBolt11(INVOICE_MAINNET_25K).amountMsat, 25_000_000n);
  assert.equal(decodeBolt11(INVOICE_NO_AMOUNT).amountMsat, null);
  // Reference values from lightning-invoice for a real maker hold invoice.
  assert.deepEqual(decodeBolt11(INVOICE_SIGNET_MAKER), {
    amountMsat: 50_000_000n,
    paymentHash:
      "6969af21a3a7485230913edffde04f45664558bbf238f52e8d7e9b469a133179",
  });
  const corrupted = INVOICE_100K.slice(0, -1) + "q";
  assert.throws(() => decodeBolt11(corrupted), /checksum/);
  assert.throws(() => decodeBolt11("bcrt1notaninvoice"), /Invalid BOLT11/);
});

test("pay-through create sends only the known fields and accepts matching terms", async () => {
  let calls = 0;
  const client = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async (url, init) => {
      calls++;
      assert.equal(url, "https://maker.example/v2/swap/pay");
      assert.equal(init.redirect, "error");
      assert.equal(init.headers.Authorization, undefined);
      assert.equal(init.headers["Idempotency-Key"], undefined);
      assert.deepEqual(JSON.parse(init.body), {
        destination: "bcrt1test",
        invoiceAmount: 100000,
      });
      return Response.json(created, { status: 201 });
    },
  });
  const response = await client.create({
    destination: " bcrt1test ",
    invoiceAmount: 100000,
    unexpected: "dropped",
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

test("pay-through retries a lost create response with the persisted key after a client restart", async (t) => {
  const request = {
    destination: "bcrt1test",
    invoiceAmount: 100000,
    asset: "BTC",
    pairHash: "11".repeat(32),
    webhook: { url: "https://example.com/callback" },
  };
  const idempotencyKey = "334cf4ef-414c-4af2-a347-2432aa413123";
  const apiKey = "kld_test_idempotency_fixture";
  const accepted = new Map();
  let attempts = 0;
  let creations = 0;
  const observed = [];
  const server = createServer(async (req, res) => {
    try {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const body = JSON.parse(Buffer.concat(chunks).toString());
      attempts++;
      const key = req.headers["idempotency-key"];
      const authorization = req.headers.authorization;
      observed.push({ key, authorization, body });
      assert.equal(req.method, "POST");
      assert.equal(req.url, "/v2/swap/pay");
      assert.equal(key, idempotencyKey);
      assert.equal(authorization, `Bearer ${apiKey}`);
      assert.deepEqual(body, request);
      // The supporting maker stores its outcome before sending the response.
      const scope = `${authorization}:${key}`;
      if (!accepted.has(scope)) {
        accepted.set(scope, created);
        creations++;
      }
      if (attempts === 1) {
        // Lose the HTTP response after the payment intent has been accepted.
        req.socket.destroy();
        return;
      }
      res.writeHead(201, { "Content-Type": "application/json" });
      res.end(JSON.stringify(accepted.get(scope)));
    } catch (error) {
      res.writeHead(500);
      res.end(String(error));
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(
    () =>
      new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
        server.closeAllConnections();
      }),
  );
  const options = {
    makerUrl: `http://127.0.0.1:${server.address().port}/v2`,
    apiKey,
  };
  await assert.rejects(
    new PayThroughClient(options).create(request, { idempotencyKey }),
    /fetch failed/,
  );
  assert.equal(attempts, 1, "the SDK does not retry automatically");
  assert.equal(creations, 1, "the maker accepted the lost create");

  // A fresh client loads the same saved request/key, with the same attribution.
  const recovered = await new PayThroughClient(options).create(request, {
    idempotencyKey,
  });
  assert.deepEqual(
    recovered,
    created,
    "recover the original invoice and swapAuth",
  );
  assert.equal(attempts, 2);
  assert.equal(creations, 1, "the retry did not create another swap");
  assert.deepEqual(observed[1], observed[0]);
});

for (const failure of ["timeout", "gateway 502", "lost response body"]) {
  test(`pay-through recovers the original create after ${failure} using the same key`, async () => {
    const request = { destination: "bcrt1test", invoiceAmount: 100000 };
    const idempotencyKey = "retry-same-payment-intent";
    let attempts = 0;
    let accepted;
    let creations = 0;
    const options = {
      makerUrl: "https://maker.example/v2",
      timeoutMs: 20,
      fetch: async (_url, init) => {
        attempts++;
        assert.equal(init.headers["Idempotency-Key"], idempotencyKey);
        assert.deepEqual(JSON.parse(init.body), request);
        if (!accepted) {
          accepted = created;
          creations++;
        }
        if (attempts === 1) {
          if (failure === "timeout") {
            return new Promise((_, reject) => {
              init.signal.addEventListener("abort", () =>
                reject(init.signal.reason),
              );
            });
          }
          if (failure === "gateway 502") {
            return new Response("<html>Bad Gateway</html>", { status: 502 });
          }
          const response = Response.json(accepted, { status: 201 });
          response.text = async () => {
            throw new Error("response body connection lost");
          };
          return response;
        }
        return Response.json(accepted, { status: 201 });
      },
    };
    await assert.rejects(
      new PayThroughClient(options).create(request, { idempotencyKey }),
      failure === "timeout"
        ? /timed out/
        : failure === "gateway 502"
          ? (error) =>
              error instanceof PayThroughApiError && error.status === 502
          : /response body connection lost/,
    );
    assert.equal(attempts, 1);
    const recovered = await new PayThroughClient(options).create(request, {
      idempotencyKey,
    });
    assert.deepEqual(recovered, created);
    assert.equal(attempts, 2);
    assert.equal(creations, 1);
  });
}

test("pay-through validates idempotency keys without trimming or sending invalid keys", async () => {
  let calls = 0;
  const sent = [];
  const client = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async (_url, init) => {
      calls++;
      sent.push(init.headers["Idempotency-Key"]);
      assert.equal(JSON.parse(init.body).idempotencyKey, undefined);
      return Response.json(created, { status: 201 });
    },
  });
  const request = { destination: "bcrt1test", invoiceAmount: 100000 };
  for (const idempotencyKey of [
    "",
    " leading",
    "trailing ",
    "has space",
    "has\ttab",
    "newline\n",
    "carriage\rreturn",
    "\x00",
    "\x7f",
    "non-ASCII-é",
    "x".repeat(129),
    null,
    123,
    ["first", "second"],
  ]) {
    await assert.rejects(
      client.create(request, { idempotencyKey }),
      (error) =>
        error instanceof TypeError && /idempotencyKey/.test(error.message),
    );
  }
  assert.equal(calls, 0, "invalid keys must fail before I/O");
  for (const idempotencyKey of [
    "!",
    "~".repeat(128),
    "Case-Sensitive_Key:123",
  ]) {
    await client.create(request, { idempotencyKey });
    assert.equal(sent.at(-1), idempotencyKey);
  }
  await client.create(request);
  assert.equal(sent.at(-1), undefined, "keys do not leak into a later intent");
  assert.equal(calls, 4);
});

test("pay-through exposes idempotency conflicts without retrying or replacing the key", async () => {
  const idempotencyKey = "keep-the-original-key";
  for (const code of ["idempotency_in_flight", "idempotency_mismatch"]) {
    let attempts = 0;
    const client = new PayThroughClient({
      makerUrl: "https://maker.example/v2",
      fetch: async (_url, init) => {
        attempts++;
        assert.equal(init.headers["Idempotency-Key"], idempotencyKey);
        return Response.json(
          { error: code, details: "retain the original payment intent" },
          { status: 409 },
        );
      },
    });
    await assert.rejects(
      client.create(
        { destination: "bcrt1test", invoiceAmount: 100000 },
        { idempotencyKey },
      ),
      (error) =>
        error instanceof PayThroughApiError &&
        error.status === 409 &&
        error.code === code &&
        error.details === "retain the original payment intent",
    );
    assert.equal(attempts, 1);
  }
});

test("pay-through create rejects terms that differ from the request or the invoice", async () => {
  const request = { destination: "bcrt1test", invoiceAmount: 100000 };
  const cases = [
    [{ destination: "bcrt1other" }, request, /destination/],
    [
      { invoiceAmount: 100001, invoice: INVOICE_100001 },
      request,
      /invoiceAmount/,
    ],
    [{ invoice: INVOICE_100001 }, request, /invoice amount/],
    [{ invoice: INVOICE_100K_OTHER_HASH }, request, /payment hash/],
    [{ invoice: INVOICE_NO_AMOUNT }, request, /invoice amount/],
    [
      {},
      { destination: "bcrt1test", payoutAmount: 99500 },
      /payoutAmount is 99000, expected at least 99500/,
    ],
    [{}, { ...request, asset: "L-USDT" }, /payoutAsset/],
  ];
  for (const [override, input, message] of cases) {
    await assert.rejects(
      clientReturning({ ...created, ...override }).create(input),
      message,
    );
  }
  // At least the requested payout, and the asset compared case-insensitively.
  const ok = await clientReturning(created).create({
    destination: "bcrt1test",
    payoutAmount: 98000,
    asset: "btc",
  });
  assert.equal(ok.payoutAmount, 99000);
});

test("pay-through status exposes broadcast reference and settlement state", async () => {
  const client = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async (url, init) => {
      assert.equal(init.headers["Idempotency-Key"], undefined);
      assert.equal(
        url,
        "https://maker.example/v2/swap/01j9zswapidulid0000000000",
      );
      return Response.json({
        id: "01J9ZSWAPIDULID0000000000",
        type: "reverse",
        status: "transaction.mempool",
        paymentStatus: "confirmed",
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
  // ULIDs are case-insensitive; the maker echoes them upper case.
  const status = await client.status("01j9zswapidulid0000000000");
  assert.equal(status.payout.reference, "txid");
  assert.equal(status.status, "transaction.mempool");
  await assert.rejects(
    clientReturning(
      {
        ...created,
        id: "01J9ZOTHERID0000000000000",
        type: "reverse",
        payout: { mode: "direct" },
      },
      { status: 200 },
    ).status("01J9ZSWAPIDULID0000000000"),
    /ID mismatch/,
  );
});

test("pay-through fails closed on maker errors, gateway pages and unsafe amounts", async () => {
  await assert.rejects(
    clientReturning(
      { error: "amount_out_of_limits", details: "outside [10000–1000000]" },
      { status: 422 },
    ).create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    (error) =>
      error instanceof PayThroughApiError &&
      error.status === 422 &&
      error.code === "amount_out_of_limits" &&
      error.details === "outside [10000–1000000]",
  );
  const limited = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async () =>
      new Response(JSON.stringify({ error: "rate_limited" }), {
        status: 429,
        headers: { "retry-after": "7" },
      }),
  });
  await assert.rejects(
    limited.create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    (error) => error instanceof PayThroughApiError && error.retryAfter === 7,
  );
  const gateway = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: async () =>
      new Response("<html>502 Bad Gateway</html>", { status: 502 }),
  });
  await assert.rejects(
    gateway.create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    (error) =>
      error instanceof PayThroughApiError &&
      error.status === 502 &&
      error.code === "http_error",
  );
  await assert.rejects(
    clientReturning({
      ...created,
      payoutAmount: Number.MAX_SAFE_INTEGER + 1,
    }).create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    /payoutAmount/,
  );
});

test("pay-through requests time out and honour a caller's abort signal", async () => {
  const hanging = (url, init) =>
    new Promise((_, reject) => {
      init.signal.addEventListener("abort", () => reject(init.signal.reason));
    });
  const slow = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: hanging,
    timeoutMs: 20,
  });
  await assert.rejects(
    slow.create({ destination: "bcrt1test", invoiceAmount: 100000 }),
    /timed out/,
  );
  const patient = new PayThroughClient({
    makerUrl: "https://maker.example/v2",
    fetch: hanging,
  });
  const controller = new AbortController();
  const pending = patient.status("01J9ZSWAPIDULID0000000000", {
    signal: controller.signal,
  });
  controller.abort(new Error("caller gave up"));
  await assert.rejects(pending, /caller gave up/);
});

test("pay-through calls the global fetch unbound when none is injected", async () => {
  // Browsers' window.fetch throws unless `this` is the global or undefined;
  // Node's does not, so stand in a fetch that checks like a browser's.
  const original = globalThis.fetch;
  let reached = false;
  globalThis.fetch = function (url) {
    if (this !== undefined && this !== globalThis) {
      throw new TypeError(
        "Failed to execute 'fetch' on 'Window': Illegal invocation",
      );
    }
    reached = true;
    assert.equal(
      url,
      "https://maker.example/v2/swap/01J9ZSWAPIDULID0000000000",
    );
    return Promise.resolve(
      Response.json({
        id: "01J9ZSWAPIDULID0000000000",
        type: "reverse",
        status: "swap.created",
        paymentStatus: "pending",
        failureReason: null,
        failureDetails: null,
        events: [],
        payout: { mode: "direct", destination: "bcrt1test", layer: "BTC_L1" },
      }),
    );
  };
  try {
    const client = new PayThroughClient({
      makerUrl: "https://maker.example/v2",
    });
    const status = await client.status("01J9ZSWAPIDULID0000000000");
    assert.equal(reached, true);
    assert.equal(status.status, "swap.created");
  } finally {
    globalThis.fetch = original;
  }
});
