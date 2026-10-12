import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { init, IntentsCorridor, SwapClient } from "../dist/index.node.js";

await init();
const card = JSON.parse(
  await readFile(
    new URL("../../tests/fixtures/corridor-v1/card.json", import.meta.url),
  ),
);

test("corridor card declarations preserve nullable results and amount strings", () => {
  execFileSync(
    process.execPath,
    [
      fileURLToPath(
        new URL("../node_modules/typescript/bin/tsc", import.meta.url),
      ),
      "--noEmit",
      "--strict",
      "--skipLibCheck",
      "--module",
      "NodeNext",
      "--moduleResolution",
      "NodeNext",
      "--target",
      "ES2020",
      fileURLToPath(new URL("./corridor-card.types.mts", import.meta.url)),
    ],
    { stdio: "inherit" },
  );
});

test("corridor card uses the sibling route and preserves maker amount strings", async () => {
  const requests = [];
  let status = 200;
  let body = JSON.stringify({ ...card, future_field: "ignored" });
  const server = createServer((req, res) => {
    requests.push({ method: req.method, url: req.url, headers: req.headers });
    res.writeHead(status, { "content-type": "application/json" });
    res.end(body);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = SwapClient.forKaleidoMaker({
    makerUrl: `http://127.0.0.1:${server.address().port}/prefix/v2/`,
    apiKey: "kld_test_01KZZYB138E7C3HZX7Q1YBGAQG_s3cr3t-Ab_Cd0123456789xyz",
  });
  try {
    const corridor = new IntentsCorridor(client);
    assert.deepEqual(await client.corridorCard(), card);
    assert.deepEqual(await corridor.card(), card);
    assert.equal(
      (await corridor.card()).markets[0].max_quote_amount,
      "9007199254740993",
    );
    status = 404;
    body = "no live card";
    assert.equal(await corridor.card(), null);
    status = 503;
    body = JSON.stringify({ error: "unavailable" });
    await assert.rejects(corridor.card(), { code: "HTTPStatusNotSuccess" });
    status = 200;
    for (const invalid of [
      "not JSON",
      "{}",
      JSON.stringify({ ...card, markets: "wrong" }),
    ]) {
      body = invalid;
      await assert.rejects(corridor.card(), {
        code: "HTTPResponseBodyInvalid",
      });
    }
    assert.ok(
      requests.every((r) => r.method === "GET" && r.url === "/prefix/v1/card"),
    );
    assert.ok(
      requests.every((r) =>
        r.headers.authorization?.startsWith("Bearer kld_test_"),
      ),
    );
  } finally {
    client.free();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("corridor card rejects a base without /v2 before making a request", async () => {
  const client = new SwapClient("https://example.invalid");
  try {
    await assert.rejects(client.corridorCard(), /does not end in \/v2/);
  } finally {
    client.free();
  }
});
