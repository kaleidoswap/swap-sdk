import assert from "node:assert/strict";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import test from "node:test";

import { BtcLikeTransaction, init } from "../dist/index.node.js";
import { BtcLikeTransaction as BrowserTransaction } from "../dist/index.js";

const fixtures = JSON.parse(
  await readFile(
    new URL(
      "../../tests/fixtures/transactions-v1/saved-transactions.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
await init();

for (const [name, chain] of [
  ["bitcoin", "bitcoin"],
  ["bitcoinLegacy", "bitcoin"],
  ["liquid", "liquid"],
]) {
  test(`fromHex preserves ${name} bytes and txid across serialization and reload`, () => {
    assert.equal(BtcLikeTransaction, BrowserTransaction);
    const fixture = fixtures[name];
    const original = BtcLikeTransaction.fromHex(fixture.hex, chain);
    const saved = JSON.parse(JSON.stringify({ hex: original.hex(), chain }));
    original.free();
    const restored = BtcLikeTransaction.fromHex(saved.hex, saved.chain);
    assert.equal(restored.hex(), fixture.hex);
    assert.equal(restored.txid(), fixture.txid);
    restored.free();
    const uppercase = BtcLikeTransaction.fromHex(
      fixture.hex.toUpperCase(),
      chain,
    );
    assert.equal(uppercase.hex(), fixture.hex);
    assert.equal(uppercase.txid(), fixture.txid);
    uppercase.free();
  });
}

test("fromHex rejects invalid arguments, truncated transactions, trailing bytes and the wrong chain", () => {
  const invalid = (call, argument) =>
    assert.throws(call, (error) => {
      assert.ok(error instanceof Error);
      assert.equal(error.code, "InvalidArgument");
      assert.match(error.message, argument);
      return true;
    });
  for (const hex of [undefined, null, 12, {}, new Uint8Array([0])]) {
    invalid(() => BtcLikeTransaction.fromHex(hex, "bitcoin"), /hex/);
  }
  for (const chain of [
    undefined,
    null,
    12,
    {},
    "",
    "BTC",
    "regtest",
    "Bitcoin",
  ]) {
    invalid(
      () => BtcLikeTransaction.fromHex(fixtures.bitcoin.hex, chain),
      /chain/,
    );
  }
  for (const chain of ["bitcoin", "liquid"]) {
    const hex = fixtures[chain].hex;
    // Encode the one-input count with an unnecessarily wide CompactSize.
    // Parsing must not silently normalize it and change the saved bytes.
    const countOffset = chain === "bitcoin" ? 12 : 10;
    const noncanonical =
      hex.slice(0, countOffset) + "fd0100" + hex.slice(countOffset + 2);
    invalid(() => BtcLikeTransaction.fromHex(noncanonical, chain), /hex/);
    for (const bad of [
      "",
      "zz",
      "0",
      "0x" + hex,
      " " + hex,
      hex.slice(0, -2),
      hex + "00",
    ]) {
      invalid(() => BtcLikeTransaction.fromHex(bad, chain), /hex/);
    }
    const other = chain === "bitcoin" ? "liquid" : "bitcoin";
    invalid(() => BtcLikeTransaction.fromHex(hex, other), /hex/);
  }
});

for (const chain of ["bitcoin", "liquid"]) {
  test(`a lost ${chain} broadcast can be retried with the exact saved bytes`, async (t) => {
    const fixture = fixtures[chain];
    const requests = [];
    const server = createServer(async (req, res) => {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      requests.push({
        method: req.method,
        path: req.url,
        hex: Buffer.concat(chunks).toString(),
      });
      if (requests.length === 1) {
        // The backend accepted the bytes but its response never reached the
        // caller. The caller must recover the transaction, not sign another.
        req.socket.destroy();
      } else {
        res.writeHead(200, { "Content-Type": "text/plain" });
        res.end(fixture.txid);
      }
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    t.after(
      () =>
        new Promise((resolve) => {
          server.close(resolve);
          server.closeAllConnections();
        }),
    );
    const endpoint = `http://127.0.0.1:${server.address().port}`;
    const original = BtcLikeTransaction.fromHex(fixture.hex, chain);
    const saved = JSON.parse(
      JSON.stringify({
        hex: original.hex(),
        chain,
        network: "regtest",
        endpoint,
      }),
    );
    const broadcast = (tx) =>
      tx.broadcast(
        saved.network,
        saved.chain === "bitcoin" ? saved.endpoint : undefined,
        saved.chain === "liquid" ? saved.endpoint : undefined,
        2n,
      );
    await assert.rejects(broadcast(original));
    assert.equal(requests.length, 1, "broadcast does not retry automatically");
    original.free();

    const restored = BtcLikeTransaction.fromHex(saved.hex, saved.chain);
    assert.equal(restored.txid(), fixture.txid);
    assert.equal(await broadcast(restored), fixture.txid);
    assert.equal(
      restored.hex(),
      saved.hex,
      "broadcast must not mutate the transaction",
    );
    restored.free();
    assert.deepEqual(
      requests,
      [1, 2].map(() => ({ method: "POST", path: "/tx", hex: fixture.hex })),
    );
  });
}
