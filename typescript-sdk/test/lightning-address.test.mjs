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
      { code: "HTTP" },
    );
  } finally {
    globalThis.fetch = originalFetch;
  }
});
