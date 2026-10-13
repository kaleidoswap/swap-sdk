import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";
import { init, SwapClient } from "../dist/index.node.js";

await init();

test("public/browser/raw WASM swap payloads compile without any", () => {
  const file = fileURLToPath(
    new URL("fixtures/swap-types.mts", import.meta.url),
  );
  const program = ts.createProgram([file], {
    noEmit: true,
    strict: true,
    skipLibCheck: false,
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.NodeNext,
    moduleResolution: ts.ModuleResolutionKind.NodeNext,
  });
  const errors = ts.getPreEmitDiagnostics(program);
  assert.equal(
    errors.length,
    0,
    ts.formatDiagnosticsWithColorAndContext(errors, {
      getCurrentDirectory: () => process.cwd(),
      getCanonicalFileName: (name) => name,
      getNewLine: () => "\n",
    }),
  );
});

test("a missing nested WASM declaration is rejected instead of becoming any", () => {
  const directory = mkdtempSync(join(tmpdir(), "swap-nested-"));
  try {
    const source = readFileSync(
      new URL("../vendor/bindings_wasm.d.ts", import.meta.url),
      "utf8",
    );
    const damaged = source.replace(/export type SwapTree = [^;]+;/, "");
    assert.notEqual(
      damaged,
      source,
      "mutation must remove the real nested declaration",
    );
    const file = join(directory, "bindings.d.ts");
    writeFileSync(file, damaged);
    const program = ts.createProgram([file], {
      noEmit: true,
      strict: true,
      skipLibCheck: false,
      target: ts.ScriptTarget.ES2022,
    });
    assert.ok(
      ts
        .getPreEmitDiagnostics(program)
        .some(
          (d) =>
            d.code === 2304 &&
            ts
              .flattenDiagnosticMessageText(d.messageText, " ")
              .includes("SwapTree"),
        ),
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("maker compatibility gate detects new, removed and mistyped fields", () => {
  const script = fileURLToPath(
    new URL("../scripts/check-swap-schema.mjs", import.meta.url),
  );
  const schema = JSON.parse(
    readFileSync(
      new URL("../../tests/fixtures/maker-swap-schema.json", import.meta.url),
      "utf8",
    ),
  );
  execFileSync(process.execPath, [script]);
  const directory = mkdtempSync(join(tmpdir(), "swap-schema-"));
  try {
    for (const mutate of [
      (s) => {
        s.schemas.RestoredSwap.properties.newField = { type: "string" };
      },
      (s) => {
        delete s.schemas.SwapStatus.properties.paymentStatus;
      },
      (s) => {
        s.schemas.RestoreClaimDetails.properties.keyIndex = { type: "string" };
      },
      (s) => {
        s.schemas.RestoredSwapType.enum.push("unknown-kind");
      },
      (s) => {
        s.routes["/v2/swap/restore"].responses["200"] = { type: "string" };
      },
      (s) => {
        s.schemas.RestoreClaimDetails.required =
          s.schemas.RestoreClaimDetails.required.filter(
            (field) => field !== "keyIndex",
          );
      },
    ]) {
      const changed = structuredClone(schema);
      mutate(changed);
      const file = join(directory, "schema.json");
      writeFileSync(file, JSON.stringify(changed));
      const result = spawnSync(process.execPath, [script, file], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, "accepted schema drift");
      assert.match(result.stderr, /AssertionError/);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("typed recovery/status DTOs preserve maker metadata and exact 64-bit integers", async () => {
  const entry = {
    id: "restored-id",
    type: "reverse",
    status: "swap.created",
    createdAt: 1,
    from: "BTC",
    to: "BTC",
    preimageHash: "ab".repeat(32),
    claimDetails: {
      type: "utxo",
      amount: 1,
      keyIndex: 7,
      lockupAddress: "bcrt1pfixture",
      serverPublicKey: "02".repeat(33),
      timeoutBlockHeight: 144,
      tree: {
        claimLeaf: { output: "00", version: 192 },
        refundLeaf: { output: "00", version: 192 },
      },
      preimageHash: "ab".repeat(32),
    },
  };
  const status = {
    id: "restored-id",
    type: "reverse",
    status: "transaction.mempool",
    paymentStatus: "paid",
    failureReason: null,
    events: [
      {
        kind: "payment_attempted",
        ts: 1,
        data: { attempt: 2, certainty: "unknown" },
      },
    ],
    payout: {
      mode: "direct",
      destination: "tb1p...",
      layer: "bitcoin",
      reference: "txid",
    },
  };
  let requests = 0;
  const server = createServer(async (req, res) => {
    requests += 1;
    await Array.fromAsync(req);
    const body =
      req.url === "/swap/restore"
        ? [entry]
        : req.url === "/swap/restore/index"
          ? { index: -1 }
          : status;
    res.setHeader("Content-Type", "application/json");
    res.end(
      JSON.stringify(body).replace(
        /"(createdAt|amount|ts)":1/g,
        '"$1":9007199254740993',
      ),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = new SwapClient(`http://127.0.0.1:${server.address().port}`);
  try {
    const [restored] = await client.swapRestore("xpub", "m");
    assert.equal(restored.createdAt, 9007199254740993n);
    assert.equal(restored.preimageHash, entry.preimageHash);
    assert.equal(restored.claimDetails.type, "utxo");
    assert.equal(restored.claimDetails.amount, 9007199254740993n);
    assert.equal(restored.claimDetails.timeoutBlockHeight, 144);
    assert.equal(restored.refundDetails, undefined);
    assert.equal((await client.swapRestoreIndex("xpub", "m")).index, -1n);
    const result = await client.swap("restored-id");
    assert.equal(result.events[0].ts, 9007199254740993n);
    assert.equal(result.events[0].data.attempt, 2n);
    assert.equal(result.events[0].data.certainty, "unknown");
    assert.deepEqual(result.payout, status.payout);
    assert.equal(result.failureReason, undefined);
    const beforeInvalidAmount = requests;
    await assert.rejects(
      client.createReverseSwap("regtest", {
        from: "BTC",
        to: "BTC",
        claimPublicKey:
          "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        preimageHash: "ab".repeat(32),
        invoiceAmount: 9007199254740992,
      }),
      (error) => error.code === "InvalidArgument",
    );
    assert.equal(
      requests,
      beforeInvalidAmount,
      "unsafe Number reached the maker",
    );
  } finally {
    client.free();
    await new Promise((resolve) => server.close(resolve));
  }
});
