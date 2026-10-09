import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import test from "node:test";
import {
  init,
  SwapClient,
  SwapScript,
  getRgbRefundPartialSig,
} from "../dist/index.node.js";

// Synthetic Bitcoin/PSBT vectors. Actual RGB proof validation belongs to rgb-lib.
const vectors = JSON.parse(
  await readFile(
    new URL("../../bindings/tests/fixtures/rgb-spends.json", import.meta.url),
    "utf8",
  ),
);
await init();

function hasCode(code, message) {
  return (error) => {
    assert.ok(error instanceof Error);
    assert.equal(error.code, code);
    assert.match(error.message, message);
    return true;
  };
}

test("RGB create pins, colored claims and caller-funded refunds cross the wasm boundary", async () => {
  let reply;
  const requests = [];
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    requests.push(JSON.parse(body));
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify(reply));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${server.address().port}/v2`;
  const client = new SwapClient(url, undefined);
  try {
    for (const reverse of [true, false]) {
      const vector = vectors.swaps[reverse ? "reverse" : "submarine"];
      const request = reverse
        ? {
            from: "BTC",
            to: "USDT-RGB",
            claimPublicKey: vectors.publicKeyHex,
            preimageHash: vectors.preimageHash,
            invoiceAmount: 1005n,
          }
        : {
            from: "USDT-RGB",
            to: "BTC",
            refundPublicKey: vectors.publicKeyHex,
            invoice: vectors.invoice,
          };
      const create = (pin) =>
        reverse
          ? client.createReverseSwap("regtest", request, pin)
          : client.createSubmarineSwap("regtest", request, pin);
      for (const pin of [undefined, "", "   "]) {
        const before = requests.length;
        await assert.rejects(
          () => create(pin),
          hasCode("InvalidArgument", /rgbContractId/),
        );
        assert.equal(
          requests.length,
          before,
          "missing pin must fail before POST",
        );
      }
      await assert.rejects(
        () => create(123),
        hasCode("InvalidArgument", /rgbContractId/),
      );
      reply = structuredClone(vector.response);
      const response = await create(vectors.contractId);
      assert.equal(response.rgb.amount, 1005n);
      assert.equal(response.rgb.htlcSat, reverse ? 1521n : 1000n);
      assert.equal(response.rgb.assetId, vectors.contractId);
      assert.equal(
        "rgbContractId" in requests.at(-1),
        false,
        "the pin is local",
      );
      if (!reverse) {
        for (const invalid of [-1n, 1n << 64n, "5000000", 5000000]) {
          const before = requests.length;
          await assert.rejects(
            () =>
              client.createSubmarineSwap(
                "regtest",
                request,
                vectors.contractId,
                invalid,
              ),
            hasCode("InvalidArgument", /rgbMaxHtlcSat/),
          );
          assert.equal(
            requests.length,
            before,
            "invalid cap must fail before POST",
          );
        }
        reply.rgb.htlcSat = 5000000;
        await assert.rejects(
          () => create(vectors.contractId),
          hasCode("Protocol", /collateral cap/),
        );
        const approved = await client.createSubmarineSwap(
          "regtest",
          request,
          vectors.contractId,
          5000000n,
        );
        assert.equal(approved.rgb.htlcSat, 5000000n);
        assert.equal(
          "rgbMaxHtlcSat" in requests.at(-1),
          false,
          "collateral cap is local",
        );
        reply = structuredClone(vector.response);
      }
      reply.rgb.assetId = "rgb:substituted-contract";
      await assert.rejects(
        () => create(vectors.contractId),
        hasCode("Protocol", /contract/i),
      );

      const script = reverse
        ? SwapScript.fromReverse(
            "bitcoin",
            "regtest",
            response,
            vectors.publicKeyHex,
          )
        : SwapScript.fromSubmarine(
            "bitcoin",
            "regtest",
            response,
            vectors.publicKeyHex,
          );
      const params = {
        outputAddress: vectors.payoutAddress,
        funding: reverse
          ? { kind: "htlcValue", feeRateSatVb: 5n }
          : { kind: "callerInputs" },
        maxFee: 10000n,
        swapId: response.id,
        makerBaseUrl: url,
        network: "regtest",
        bitcoinEsploraUrl: url,
        lockupTxHex: vector.lockTxHex,
      };
      const { lockupTxHex, ...unpinned } = params;
      await assert.rejects(
        () =>
          reverse
            ? script.prepareRgbClaim(unpinned)
            : script.prepareRgbRefund(unpinned),
        hasCode("InvalidArgument", /lockupTxHex/),
      );
      assert.equal(lockupTxHex, vector.lockTxHex);
      let spend = reverse
        ? await script.prepareRgbClaim(params)
        : await script.prepareRgbRefund(params);
      if (!reverse) {
        await assert.rejects(
          () => script.prepareRgbCooperativeRefund(params),
          hasCode("Protocol", /advertise/),
        );
      }
      let finalized;
      try {
        const template = spend.template();
        assert.equal(template.amount, 1005n);
        assert.equal(template.paymentValue, reverse ? 546n : 1000n);
        assert.equal(template.commitmentOutputIndex, 0);
        assert.equal(template.paymentOutputIndex, 1);
        if (!reverse) {
          assert.equal(template.requiresFunding, true);
          const funded = spend.fund(vector.fundedPsbt);
          assert.equal(spend.template().requiresFunding, true);
          spend.free();
          spend = funded;
          assert.equal(spend.template().psbt, vector.fundedPsbt);
          assert.equal(spend.template().swapInputIndex, 1);
          assert.equal(spend.template().requiresFunding, false);
        }
        const colored = {
          psbt: vector.coloredPsbt,
          allocations: [
            { assetId: vectors.contractId, vout: 1, amount: 1005n },
          ],
        };
        const finalize = () =>
          reverse
            ? spend.finalizeClaim(
                colored,
                vectors.secretKeyHex,
                vectors.preimageHex,
              )
            : spend.finalizeRefund(colored, vectors.secretKeyHex);
        colored.allocations[0].amount = 1004n;
        assert.throws(finalize, hasCode("Protocol", /allocation/i));
        colored.allocations[0].amount = 1005n;
        finalized = finalize();
        assert.notEqual(
          finalized.psbt,
          colored.psbt,
          "the HTLC witness was added",
        );
        assert.equal(finalized.swapInputIndex, reverse ? 0 : 1);
        if (reverse) {
          assert.match(finalized.transaction.txid(), /^[0-9a-f]{64}$/);
          assert.ok(finalized.transaction.hex().includes(vectors.preimageHex));
        } else {
          assert.equal(
            finalized.transaction,
            null,
            "wallet fee input is still unsigned",
          );
          await assert.rejects(
            () =>
              script.prepareRgbRefund({
                ...params,
                funding: { kind: "htlcValue", feeRateSatVb: 5n },
              }),
            hasCode("rgb_fee_input_required", /fee/i),
          );
        }
        await assert.rejects(
          () =>
            script.constructRefund({
              ...params,
              keysSecretHex: vectors.secretKeyHex,
              feeAbsoluteSat: 100n,
            }),
          hasCode("Protocol", /RGB/i),
        );
      } finally {
        finalized?.transaction?.free();
        spend.free();
        script.free();
      }
    }
    const before = requests.length;
    await assert.rejects(
      () =>
        client.createChainSwap("regtest", {
          from: "BTC",
          to: "USDT-RGB",
          preimageHash: vectors.preimageHash,
          claimPublicKey: vectors.publicKeyHex,
          refundPublicKey: vectors.publicKeyHex,
        }),
      hasCode("InvalidArgument", /chain swaps are unsupported/),
    );
    await assert.rejects(
      () =>
        client.createSubmarineSwap(
          "regtest",
          {
            from: "BTC",
            to: "USDT-RGB",
            invoice: vectors.invoice,
            refundPublicKey: vectors.publicKeyHex,
          },
          vectors.contractId,
        ),
      hasCode("InvalidArgument", /direction/),
    );
    await assert.rejects(
      () =>
        client.createReverseSwap(
          "regtest",
          {
            from: "USDT-RGB",
            to: "BTC",
            preimageHash: vectors.preimageHash,
            claimPublicKey: vectors.publicKeyHex,
          },
          vectors.contractId,
        ),
      hasCode("InvalidArgument", /direction/),
    );
    assert.equal(requests.length, before);
  } finally {
    client.free();
    server.closeAllConnections();
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

test("RGB refund transport sends the versioned body and per-swap credential", async () => {
  const received = [];
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    received.push({
      path: req.url,
      auth: req.headers["x-swap-auth"],
      body: JSON.parse(body),
    });
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(
      JSON.stringify({
        sessionId: "11".repeat(32),
        requestHash: "22".repeat(32),
        pubNonce: "33".repeat(66),
        partialSignature: "44".repeat(32),
      }),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = new SwapClient(
    `http://127.0.0.1:${server.address().port}/v2`,
    undefined,
  );
  const request = {
    protocol: "rgb-coop-refund-v1",
    psbt: "colored-psbt",
    index: 1,
    pubNonce: "55".repeat(66),
    sessionId: "11".repeat(32),
  };
  try {
    await assert.rejects(
      () => getRgbRefundPartialSig(client, "swap-id", request, ""),
      /credential|swapAuth/i,
    );
    assert.equal(received.length, 0);
    const response = await getRgbRefundPartialSig(
      client,
      "swap-id",
      request,
      "test-credential",
    );
    assert.equal(response.sessionId, request.sessionId);
    assert.deepEqual(received, [
      {
        path: "/v2/swap/submarine/swap-id/refund",
        auth: "test-credential",
        body: request,
      },
    ]);
  } finally {
    client.free();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});
