import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import {
  EsploraDataError,
  esploraFeeEstimates,
  esploraTipHeight,
} from "../dist/chain-data.js";

test("chain data helpers are available through both public entry points", async () => {
  const node = await import("../dist/index.node.js");
  const browser = await import("../dist/index.js");
  assert.equal(node.esploraFeeEstimates, esploraFeeEstimates);
  assert.equal(browser.esploraTipHeight, esploraTipHeight);
});

async function serverFor(t, handler) {
  const server = createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  });
  return `http://127.0.0.1:${server.address().port}`;
}

test("explicit Bitcoin/Liquid endpoints preserve fee targets, units and tip heights", async (t) => {
  const requests = [];
  const base = await serverFor(t, (req, res) => {
    requests.push({ url: req.url, method: req.method, headers: req.headers });
    if (req.url.endsWith("/fee-estimates")) {
      res.end(
        req.url.startsWith("/bitcoin") ? '{"2":3.5,"6":1.25}' : '{"1":0.1}',
      );
    } else {
      res.end(req.url.startsWith("/bitcoin") ? "123456\n" : "0\n");
    }
  });
  const fees = await esploraFeeEstimates(`${base}/bitcoin/api/`);
  assert.deepEqual(fees, { 2: 3.5, 6: 1.25 });
  assert.equal(fees[3], undefined);
  assert.equal(Object.isFrozen(fees), true);
  assert.deepEqual(await esploraFeeEstimates(`${base}/liquid/api`), { 1: 0.1 });
  assert.equal(await esploraTipHeight(`${base}/bitcoin/api`), 123456);
  assert.equal(await esploraTipHeight(`${base}/liquid/api`), 0);
  assert.deepEqual(
    requests.map((r) => r.url),
    [
      "/bitcoin/api/fee-estimates",
      "/liquid/api/fee-estimates",
      "/bitcoin/api/blocks/tip/height",
      "/liquid/api/blocks/tip/height",
    ],
  );
  assert.ok(
    requests.every(
      (r) =>
        r.method === "GET" && !r.headers.authorization && !r.headers.cookie,
    ),
  );
});

test("missing or malformed chain data never becomes a default fee or height", async (t) => {
  let status = 200;
  let body = "";
  let calls = 0;
  const base = await serverFor(t, (_req, res) => {
    calls++;
    res.writeHead(status);
    res.end(body);
  });
  for (const invalid of [
    "not JSON",
    "null",
    "[]",
    "{}",
    '{"1":0}',
    '{"1":-1}',
    '{"1":"2"}',
    '{"1":1e999}',
    '{"0":1}',
    '{"01":1}',
    '{"1.5":1}',
    '{"x":1}',
    '{"4294967296":1}',
  ]) {
    body = invalid;
    await assert.rejects(esploraFeeEstimates(base), {
      code: "HTTPResponseBodyInvalid",
      status: 200,
    });
  }
  for (const invalid of [
    "",
    "-1",
    "01",
    "1.5",
    "1e3",
    "NaN",
    "<html>oops</html>",
    "4294967296",
    "9007199254740993",
  ]) {
    body = invalid;
    await assert.rejects(esploraTipHeight(base), {
      code: "HTTPResponseBodyInvalid",
    });
  }
  for (const failure of [404, 429, 503]) {
    status = failure;
    const before = calls;
    await assert.rejects(
      esploraTipHeight(base),
      (error) =>
        error instanceof EsploraDataError &&
        error.code === "HTTPStatusNotSuccess" &&
        error.status === failure,
    );
    assert.equal(calls, before + 1, "no automatic retry or fallback");
  }
  status = 201;
  body = "{}";
  await assert.rejects(esploraTipHeight(base), {
    code: "HTTPResponseBodyInvalid",
    status: 201,
  });
});

test("deadlines include the body and caller cancellation prevents unnecessary I/O", async (t) => {
  let calls = 0;
  const base = await serverFor(t, (_req, res) => {
    calls++;
    res.writeHead(200);
    res.write("123"); // Deliberately never complete the body.
  });
  await assert.rejects(esploraTipHeight(base, { timeoutMs: 100 }), (error) =>
    ["TimeoutError", "AbortError"].includes(error.name),
  );
  assert.equal(calls, 1);
  const controller = new AbortController();
  const reason = new Error("wallet cancelled");
  controller.abort(reason);
  await assert.rejects(
    esploraTipHeight(base, { signal: controller.signal }),
    (error) => error === reason,
  );
  assert.equal(calls, 1);
});

test("caller cancellation interrupts an in-progress body read", async (t) => {
  let received;
  const ready = new Promise((resolve) => {
    received = resolve;
  });
  const base = await serverFor(t, (_req, res) => {
    res.writeHead(200);
    res.write("123");
    received();
  });
  const controller = new AbortController();
  const pending = esploraTipHeight(base, { signal: controller.signal });
  await ready;
  controller.abort();
  await assert.rejects(pending, { name: "AbortError" });
});

test("URL and timeout validation precede I/O; fetch gets no maker authentication", async () => {
  let calls = 0;
  const request = async function (url, options) {
    assert.equal(this, undefined);
    calls++;
    assert.equal(url, "https://esplora.example/api/blocks/tip/height");
    assert.equal(options.redirect, "error");
    assert.equal(options.credentials, "omit");
    assert.deepEqual(options.headers, { Accept: "text/plain" });
    return new Response("42");
  };
  for (const url of [
    "not a URL",
    "file:///tmp/esplora",
    "https://user:secret@esplora.example",
    "https://esplora.example?key=secret",
    "https://esplora.example#fragment",
  ]) {
    await assert.rejects(esploraTipHeight(url, { fetch: request }), TypeError);
  }
  for (const timeoutMs of [0, -1, 0.5, NaN, Infinity, 2147483648]) {
    await assert.rejects(
      esploraTipHeight("https://esplora.example/api", {
        fetch: request,
        timeoutMs,
      }),
      TypeError,
    );
  }
  assert.equal(calls, 0);
  assert.equal(
    await esploraTipHeight("https://esplora.example/api/", { fetch: request }),
    42,
  );
  assert.equal(calls, 1);
  const failure = new TypeError("connection lost");
  await assert.rejects(
    esploraTipHeight("https://esplora.example/api", {
      fetch: async () => {
        throw failure;
      },
    }),
    (error) => error === failure,
  );
});
