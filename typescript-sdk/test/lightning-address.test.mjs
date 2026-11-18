import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { init, resolveLightningAddress } from "../dist/index.node.js";

await init();
const fixture = JSON.parse(
  await readFile(
    new URL("../../tests/fixtures/lnurl-v1/invoice.json", import.meta.url),
  ),
);

test("Lightning-address and LNURL lookups reuse the Rust resolver with satoshi conversion", async () => {
  const browser = await import("../dist/index.js");
  assert.equal(browser.resolveLightningAddress, resolveLightningAddress);
  const originalFetch = globalThis.fetch;
  const requests = [];
  globalThis.fetch = async (request) => {
    assert.equal(request.redirect, "error");
    assert.equal(request.credentials, "omit");
    const url = new URL(request.url);
    requests.push(url.href);
    const body =
      url.hostname === "callback.example"
        ? { pr: fixture.invoice }
        : {
            tag: "payRequest",
            callback: "https://callback.example/invoice?token=kept",
            minSendable: 1000,
            maxSendable: 200000000,
            metadata: '[["text/plain","test"]]',
          };
    const response = new Response(JSON.stringify(body), {
      headers: { "content-type": "application/json" },
    });
    // Real fetch responses carry the final URL, which reqwest validates.
    Object.defineProperty(response, "url", { value: request.url });
    return response;
  };
  try {
    assert.equal(
      await resolveLightningAddress("alice@pay.example", 100000n),
      fixture.invoice,
    );
    assert.equal(
      await resolveLightningAddress(fixture.lnurl.toUpperCase(), 100000n),
      fixture.invoice,
    );
    assert.deepEqual(requests, [
      "https://pay.example/.well-known/lnurlp/alice",
      "https://callback.example/invoice?token=kept&amount=100000000",
      fixture.url,
      "https://callback.example/invoice?token=kept&amount=100000000",
    ]);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("resolver rejects invalid units, service limits, invoices and transport failures", async () => {
  const originalFetch = globalThis.fetch;
  let requests = 0;
  let minSendable = 1000;
  let invoice = fixture.invoice;
  let status = 200;
  globalThis.fetch = async (request) => {
    requests++;
    const body =
      new URL(request.url).hostname === "callback.example"
        ? { pr: invoice }
        : {
            tag: "payRequest",
            callback: "https://callback.example/invoice",
            minSendable,
            maxSendable: 200000000,
            metadata: '[["text/plain","test"]]',
          };
    const response = new Response(JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    });
    // Real fetch responses carry the final URL, which reqwest validates.
    Object.defineProperty(response, "url", { value: request.url });
    return response;
  };
  try {
    for (const amount of [
      0n,
      -1n,
      18446744073709552n,
      1n << 64n,
      100000,
      "100000",
      undefined,
    ]) {
      await assert.rejects(
        resolveLightningAddress("alice@pay.example", amount),
        { code: "InvalidArgument" },
      );
    }
    await assert.rejects(resolveLightningAddress(42, 1n), {
      code: "InvalidArgument",
    });
    await assert.rejects(resolveLightningAddress("invalid-address", 1n));
    assert.equal(requests, 0);
    minSendable = 100000001;
    await assert.rejects(resolveLightningAddress("alice@pay.example", 100000n));
    assert.equal(requests, 1, "out-of-range amount must not call the callback");
    minSendable = 1000;
    await assert.rejects(
      resolveLightningAddress("alice@pay.example", 100001n),
      /doesn't match requested amount/,
    );
    invoice = "not-a-bolt11-invoice";
    await assert.rejects(resolveLightningAddress("alice@pay.example", 100000n));
    status = 503;
    await assert.rejects(
      resolveLightningAddress("alice@pay.example", 100000n),
      { code: "HTTPStatusNotSuccess" },
    );
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("resolver enforces secure endpoints, metadata binding and provider error reasons", async () => {
  const originalFetch = globalThis.fetch;
  let callback = "https://callback.example/invoice";
  let metadata = '[["text/plain","test"]]';
  let errorAt;
  let count = 0;
  globalThis.fetch = async (request) => {
    count++;
    assert.equal(request.redirect, "error");
    assert.equal(request.credentials, "omit");
    const stage =
      new URL(request.url).hostname === "callback.example"
        ? "callback"
        : "service";
    const body =
      errorAt === stage
        ? { status: "ERROR", reason: "Provider cannot serve this request" }
        : stage === "callback"
          ? { pr: fixture.invoice }
          : {
              tag: "payRequest",
              callback,
              minSendable: 1000,
              maxSendable: 200000000,
              metadata,
            };
    const response = new Response(JSON.stringify(body));
    Object.defineProperty(response, "url", { value: request.url });
    return response;
  };
  try {
    await assert.rejects(
      resolveLightningAddress(fixture.insecureLnurl, 100000n),
      /HTTPS/,
    );
    assert.equal(count, 0);
    callback = "http://callback.example/invoice";
    await assert.rejects(
      resolveLightningAddress("alice@pay.example", 100000n),
      /HTTPS/,
    );
    assert.equal(count, 1, "insecure callback must not be contacted");
    callback = "https://callback.example/invoice";
    metadata = '[["text/plain","different payee"]]';
    await assert.rejects(
      resolveLightningAddress("alice@pay.example", 100000n),
      /description_hash/,
    );
    metadata = '[["text/plain","test"]]';
    for (errorAt of ["service", "callback"])
      await assert.rejects(
        resolveLightningAddress("alice@pay.example", 100000n),
        /Provider cannot serve/,
      );
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test(
  "WASM resolver bounds stalled bodies and rejects redirected responses",
  { timeout: 5000 },
  async () => {
    const originalFetch = globalThis.fetch;
    const originalTimer = globalThis.setTimeout;
    try {
      globalThis.fetch = async (request) => {
        const response = new Response("{}");
        Object.defineProperty(response, "url", { value: request.url });
        Object.defineProperty(response, "redirected", { value: true });
        return response;
      };
      await assert.rejects(
        resolveLightningAddress("alice@pay.example", 100000n),
        /redirects/,
      );
      let armed = false;
      globalThis.setTimeout = (fn, delay, ...args) => {
        if (delay === 30000) {
          armed = true;
          delay = 10;
        }
        return originalTimer(fn, delay, ...args);
      };
      globalThis.fetch = async (request) => {
        const response = new Response(
          new ReadableStream({
            start(controller) {
              request.signal.addEventListener(
                "abort",
                () => controller.error(new Error("deadline")),
                { once: true },
              );
            },
          }),
        );
        Object.defineProperty(response, "url", { value: request.url });
        return response;
      };
      await assert.rejects(
        resolveLightningAddress("alice@pay.example", 100000n),
        /deadline/,
      );
      assert.equal(armed, true);
    } finally {
      globalThis.fetch = originalFetch;
      globalThis.setTimeout = originalTimer;
    }
  },
);
