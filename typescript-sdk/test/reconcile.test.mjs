import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import {
  canRefund,
  canClaim,
  refundSubmarineSwap,
  refundChainSwap,
  claimReverseSwap,
  claimChainSwap,
  init,
  SwapScript,
} from "../dist/index.node.js";

test("public browser and Node declarations enforce spend result and parameter contracts", () => {
  execFileSync(
    process.execPath,
    [
      fileURLToPath(
        new URL("../node_modules/typescript/bin/tsc", import.meta.url),
      ),
      "--noEmit",
      "--strict",
      "--skipLibCheck",
      "--target",
      "ES2020",
      "--module",
      "NodeNext",
      "--moduleResolution",
      "NodeNext",
      fileURLToPath(new URL("./reconcile-types.mts", import.meta.url)),
    ],
    { stdio: "pipe" },
  );
});

test("browser and Node entry points share reconciliation helpers", async () => {
  const browser = await import("../dist/index.js");
  for (const helper of [
    canRefund,
    canClaim,
    refundSubmarineSwap,
    refundChainSwap,
    claimReverseSwap,
    claimChainSwap,
  ])
    assert.equal(browser[helper.name], helper);
});

test("claim confirmation evidence cannot be replaced by a maker status", async () => {
  const { options, calls } = setup({
    status: "transaction.confirmed",
    tipHeight: 100,
  });
  for (const confirmations of [undefined, 0]) {
    assert.deepEqual(
      canClaim("transaction.confirmed", 100, timing, {
        lockupConfirmations: confirmations,
      }),
      { eligible: false, reason: "waiting-for-confirmation" },
    );
    assert.equal(
      (
        await claimReverseSwap({
          ...options,
          lockup:
            confirmations === undefined
              ? undefined
              : { ...options.lockup, confirmations },
        })
      ).kind,
      "not-yet",
    );
  }
  assert.equal(
    calls.length,
    0,
    "must not disclose the preimage or construct a claim",
  );
  assert.equal(
    canClaim("transaction.mempool", 100, timing, { lockupConfirmations: 2 })
      .eligible,
    true,
    "chain evidence outranks a stale maker mempool label",
  );
  assert.equal(
    canClaim("transaction.confirmed", 100, timing, {
      lockupConfirmations: 1,
      minConfirmations: 2,
    }).eligible,
    false,
  );
  assert.equal(
    (
      await claimReverseSwap({
        ...options,
        lockup: { ...options.lockup, confirmations: 0 },
        allowZeroConf: true,
      })
    ).kind,
    "signed",
  );
});

test("automatic claims fall back immediately after cooperation fails; explicit modes remain explicit", async () => {
  for (const chain of [false, true]) {
    const { options, calls, transaction } = setup({
      status: chain ? "transaction.server.confirmed" : "transaction.confirmed",
      tipHeight: 100,
    });
    const refused = new Error("cooperative signature refused");
    options.script.constructClaim = async (preimage, params) => {
      calls.push(params.cooperative);
      if (params.cooperative) throw refused;
      return transaction;
    };
    options.script.constructCooperativeClaim = async () => {
      calls.push(true);
      throw refused;
    };
    const input = chain
      ? { ...options, lockupScript: {}, refundKeysSecretHex: "refund-key" }
      : options;
    const helper = chain ? claimChainSwap : claimReverseSwap;
    const result = await helper(input);
    assert.equal(result.path, "script");
    assert.equal(result.transaction, transaction);
    assert.deepEqual(calls, [true, false]);
    calls.length = 0;
    await assert.rejects(
      helper({ ...input, mode: "cooperative" }),
      (error) => error === refused,
    );
    assert.deepEqual(calls, [true]);
    options.script.constructClaim = async () => {
      throw new Error("chain unavailable");
    };
    await assert.rejects(helper(input), { code: "ClaimFallbackFailed" });
  }
});

test("matured unilateral refunds cannot be vetoed by stale or unknown maker status", async () => {
  for (const status of ["swap.created", "invoice.pending", "future.state"]) {
    const { options, calls } = setup({ status, tipHeight: 200 });
    assert.deepEqual(canRefund(status, 200, timing), {
      eligible: true,
      path: "script",
    });
    assert.equal((await refundSubmarineSwap(options)).path, "script");
    assert.equal(calls[0].args[0].cooperative, false);
    assert.equal(canRefund(status, 199, timing).eligible, false);
  }
});

test("claim margins and explicit fee ceilings reject risky construction before signing", async () => {
  assert.deepEqual(
    canClaim("transaction.confirmed", 194, timing, { lockupConfirmations: 1 }),
    { eligible: false, reason: "claim-timeout" },
  );
  assert.equal(
    canClaim("transaction.confirmed", 193, timing, { lockupConfirmations: 1 })
      .eligible,
    true,
  );
  const { options, calls } = setup();
  await assert.rejects(
    refundSubmarineSwap({ ...options, feeLimits: { maxFeeSatPerVb: 0.1 } }),
    /maxFeeSatPerVb/,
  );
  await assert.rejects(
    refundSubmarineSwap({
      ...options,
      params: {
        ...options.params,
        feeSatPerVb: undefined,
        feeAbsoluteSat: 100,
      },
      feeLimits: { maxFeeAbsoluteSat: 99 },
    }),
    /maxFeeAbsoluteSat/,
  );
  assert.equal(calls.length, 0);
});

test(
  "real WASM claims pin the trusted confirmed lockup and recover from cooperative refusal",
  { timeout: 10000 },
  async () => {
    await init();
    const vector = JSON.parse(
      await readFile(
        new URL(
          "../../bindings/tests/fixtures/rgb-spends.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const response = structuredClone(vector.swaps.reverse.response);
    response.onchainAmount = BigInt(response.rgb.htlcSat);
    delete response.rgb;
    const hex = vector.swaps.reverse.lockTxHex;
    const txid = createHash("sha256")
      .update(createHash("sha256").update(Buffer.from(hex, "hex")).digest())
      .digest()
      .reverse()
      .toString("hex");
    const requests = [];
    const server = createServer(async (req, res) => {
      let body = "";
      for await (const chunk of req) body += chunk;
      requests.push({ path: req.url, body });
      // Any address discovery would expose the bug: there could be a different
      // unconfirmed HTLC here. Claims must use only the pinned raw transaction.
      res.writeHead(503, { "content-type": "application/json" });
      res.end('{"error":"signature_refused"}');
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const url = `http://127.0.0.1:${server.address().port}`;
    const script = SwapScript.fromReverse(
      "bitcoin",
      "regtest",
      response,
      vector.publicKeyHex,
    );
    const options = {
      status: "transaction.confirmed",
      tipHeight: 100,
      createResponse: response,
      script,
      preimageHex: vector.preimageHex,
      lockup: { chain: "bitcoin", hex, txid, confirmations: 1 },
      params: {
        swapId: response.id,
        outputAddress: vector.payoutAddress,
        keysSecretHex: vector.secretKeyHex,
        makerBaseUrl: `${url}/v2`,
        network: "regtest",
        bitcoinEsploraUrl: url,
        feeAbsoluteSat: 50,
      },
    };
    let result;
    try {
      assert.equal(
        (
          await claimReverseSwap({
            ...options,
            lockup: { ...options.lockup, confirmations: 0 },
          })
        ).kind,
        "not-yet",
      );
      assert.equal(requests.length, 0);
      result = await claimReverseSwap(options);
      assert.equal(result.kind, "signed");
      assert.equal(result.path, "script");
      assert.equal(requests.length, 1);
      assert.equal(requests[0].path, `/v2/swap/reverse/${response.id}/claim`);
      assert.equal(JSON.parse(requests[0].body).preimage, vector.preimageHex);
      assert.ok(result.transaction.hex().includes(vector.preimageHex));
      await assert.rejects(
        claimReverseSwap({
          ...options,
          lockup: { ...options.lockup, txid: "00".repeat(32) },
        }),
        /fallback failed/,
      );
      assert.equal(
        requests.length,
        1,
        "bad lockup identity must fail before preimage disclosure",
      );
    } finally {
      result?.transaction.free();
      script.free();
      await new Promise((resolve) => server.close(resolve));
    }
  },
);

const timing = { timeoutBlockHeight: 200n };
const chainTiming = {
  lockupDetails: { timeoutBlockHeight: 200n },
  claimDetails: { timeoutBlockHeight: 150 },
};
function setup(overrides = {}) {
  const calls = [];
  const transaction = {
    hex: () => "signed-original",
    txid: () => "original-id",
  };
  const script = Object.fromEntries(
    ["constructClaim", "constructRefund", "constructCooperativeClaim"].map(
      (method) => [
        method,
        async (...args) => {
          calls.push({ method, args });
          return transaction;
        },
      ],
    ),
  );
  return {
    calls,
    transaction,
    options: {
      status: "invoice.failedToPay",
      tipHeight: 199,
      createResponse: timing,
      script,
      preimageHex: "11".repeat(32),
      lockup: {
        chain: "bitcoin",
        hex: "00",
        txid: "11".repeat(32),
        confirmations: 1,
      },
      params: {
        swapId: "swap-1",
        swapAuth: "secret-auth",
        outputAddress: "destination",
        keysSecretHex: "key",
        network: "regtest",
        makerBaseUrl: "http://127.0.0.1/v2",
        feeSatPerVb: 0.12,
      },
      ...overrides,
    },
  };
}

test("refund policy blocks in-flight payment and unknown/terminal states even after CLTV", () => {
  for (const status of [
    "invoice.paid",
    "transaction.claimed",
    "transaction.claim.pending",
  ]) {
    assert.equal(canRefund(status, 201, timing).eligible, false, status);
  }
  assert.deepEqual(canRefund("transaction.confirmed", 199, timing), {
    eligible: false,
    reason: "waiting-for-timeout",
  });
  assert.deepEqual(canRefund("invoice.failedToPay", 199, timing), {
    eligible: true,
    path: "cooperative",
  });
  assert.deepEqual(
    canRefund("invoice.failedToPay", 199, timing, { mode: "script" }),
    { eligible: false, reason: "waiting-for-timeout" },
  );
});

test("CLTV boundary uses the lockup side and exact block heights", () => {
  for (const [height, expected] of [
    [199, false],
    [200, true],
    [201n, true],
  ]) {
    assert.equal(
      canRefund("transaction.confirmed", height, chainTiming, { kind: "chain" })
        .eligible,
      expected,
    );
  }
  assert.equal(
    canRefund("transaction.server.confirmed", 160, chainTiming, {
      kind: "chain",
    }).eligible,
    false,
  );
  for (const height of [
    -1,
    1.5,
    NaN,
    Infinity,
    "200",
    500000000n,
    Number.MAX_SAFE_INTEGER,
  ]) {
    assert.throws(() => canRefund("invoice.failedToPay", height, timing), {
      code: "InvalidArgument",
    });
  }
  assert.throws(
    () => canRefund("invoice.failedToPay", 200, { timeoutBlockHeight: 0 }),
    { code: "InvalidArgument" },
  );
  assert.throws(
    () =>
      canClaim("transaction.confirmed", 100, { timeoutBlockHeight: 500000000 }),
    { code: "InvalidArgument" },
  );
});

test("claims require the receiving-side lockup, confirmation by default and time before expiry", () => {
  assert.deepEqual(canClaim("transaction.mempool", 100, timing), {
    eligible: false,
    reason: "waiting-for-confirmation",
  });
  assert.equal(
    canClaim("transaction.mempool", 100, timing, { allowZeroConf: true })
      .eligible,
    true,
  );
  assert.equal(
    canClaim("transaction.confirmed", 100, chainTiming, { kind: "chain" })
      .eligible,
    false,
  );
  assert.equal(
    canClaim("transaction.server.confirmed", 149, chainTiming, {
      kind: "chain",
      lockupConfirmations: 1,
      claimSafetyMarginBlocks: 0,
    }).eligible,
    true,
  );
  assert.deepEqual(
    canClaim("transaction.server.confirmed", 150, chainTiming, {
      kind: "chain",
    }),
    { eligible: false, reason: "claim-timeout" },
  );
  for (const status of [
    "invoice.settled",
    "transaction.refunded",
    "transaction.claim.pending",
    "future.state",
  ])
    assert.equal(canClaim(status, 100, timing).eligible, false);
});

test("both refunds use authenticated cooperative construction before timeout and script at maturity", async () => {
  for (const helper of [refundSubmarineSwap, refundChainSwap]) {
    const { options, calls, transaction } = setup();
    const signed = await helper(options);
    assert.equal(signed.transaction, transaction);
    assert.equal(signed.path, "cooperative");
    assert.equal(calls[0].args[0].swapAuth, "secret-auth");
    assert.equal(calls[0].args[0].cooperative, true);
    const mature = await helper({ ...options, tipHeight: 200 });
    assert.equal(mature.path, "script");
    assert.equal(calls[1].args[0].cooperative, false);
    assert.equal(
      options.params.cooperative,
      undefined,
      "caller params must not be mutated",
    );
  }
});

test("not-yet and missing-auth results cause no signing or network operations", async () => {
  const { options, calls } = setup();
  assert.deepEqual(
    await refundSubmarineSwap({
      ...options,
      params: { ...options.params, swapAuth: undefined },
    }),
    { kind: "not-yet", reason: "missing-swap-auth" },
  );
  assert.equal(
    (await refundSubmarineSwap({ ...options, status: "invoice.pending" })).kind,
    "not-yet",
  );
  assert.equal(
    (await claimReverseSwap({ ...options, status: "transaction.mempool" }))
      .kind,
    "not-yet",
  );
  assert.equal(calls.length, 0);
});

test("maker refusals propagate unchanged, without a script fallback or implicit retry", async () => {
  const { options, calls } = setup();
  const refused = Object.assign(new Error("financial eligibility denied"), {
    code: "HTTP",
  });
  options.script.constructRefund = async (params) => {
    calls.push(params);
    throw refused;
  };
  await assert.rejects(
    refundSubmarineSwap(options),
    (error) => error === refused,
  );
  assert.equal(calls.length, 1);
  assert.equal(calls[0].cooperative, true);
  // Explicit cooperative mode preserves refusals even when CLTV has matured.
  await assert.rejects(
    refundSubmarineSwap({ ...options, tipHeight: 201, mode: "cooperative" }),
    (error) => error === refused,
  );
  assert.equal(calls.length, 2);
});

test("reverse and chain claims select the correct construction and distinct refund key", async () => {
  const { options, calls } = setup({
    status: "transaction.confirmed",
    tipHeight: 100,
  });
  assert.equal((await claimReverseSwap(options)).path, "cooperative");
  assert.equal(calls[0].method, "constructClaim");
  assert.equal(calls[0].args[1].cooperative, true);
  const chain = {
    ...options,
    status: "transaction.server.confirmed",
    createResponse: chainTiming,
  };
  assert.equal((await claimChainSwap(chain)).path, "script");
  assert.equal(calls[1].args[1].cooperative, false);
  const lockupScript = { distinct: "lockup script" };
  assert.equal(
    (
      await claimChainSwap({
        ...chain,
        lockupScript,
        refundKeysSecretHex: "refund-key",
      })
    ).path,
    "cooperative",
  );
  assert.deepEqual(calls[2].args.slice(2), [lockupScript, "refund-key"]);
  assert.equal(calls[2].method, "constructCooperativeClaim");
  const count = calls.length;
  assert.deepEqual(await claimChainSwap({ ...chain, mode: "cooperative" }), {
    kind: "not-yet",
    reason: "missing-cooperative-context",
  });
  assert.equal(calls.length, count);
});

test("fees must be explicit and preserve fractional rates without choosing a maker default", async () => {
  const { options, calls } = setup();
  for (const fees of [
    { feeSatPerVb: undefined },
    { feeAbsoluteSat: 5 },
    { feeSatPerVb: 0 },
    { feeSatPerVb: Infinity },
    { feeSatPerVb: -1 },
    { feeSatPerVb: undefined, feeAbsoluteSat: 1.5 },
    { feeSatPerVb: undefined, feeAbsoluteSat: Number.MAX_SAFE_INTEGER + 1 },
  ]) {
    await assert.rejects(
      refundSubmarineSwap({
        ...options,
        params: { ...options.params, ...fees },
      }),
      { code: "InvalidArgument" },
    );
  }
  assert.equal(calls.length, 0);
  await refundSubmarineSwap(options);
  assert.equal(calls[0].args[0].feeSatPerVb, 0.12);
  await refundSubmarineSwap({
    ...options,
    params: { ...options.params, feeSatPerVb: undefined, feeAbsoluteSat: 50 },
  });
  assert.equal(calls[1].args[0].feeAbsoluteSat, 50);
});

test("retry after a lost broadcast returns the original signed transaction, ignoring new fees/status", async () => {
  for (const helper of [
    refundSubmarineSwap,
    refundChainSwap,
    claimReverseSwap,
    claimChainSwap,
  ]) {
    const { options, calls } = setup({
      status:
        helper === claimReverseSwap
          ? "transaction.confirmed"
          : helper === claimChainSwap
            ? "transaction.server.confirmed"
            : "invoice.failedToPay",
      tipHeight: 100,
    });
    const first = await helper(options);
    assert.equal(first.kind, "signed");
    const retry = await helper({
      ...options,
      status: "future.state",
      tipHeight: 999,
      params: { ...options.params, feeSatPerVb: undefined },
      previous: first,
    });
    assert.equal(retry, first);
    assert.equal(retry.transaction.hex(), "signed-original");
    assert.equal(retry.transaction.txid(), "original-id");
    assert.equal(calls.length, 1, "retry must not reconstruct or re-sign");
    await assert.rejects(
      helper({
        ...options,
        params: { ...options.params, swapId: "other" },
        previous: first,
      }),
      { code: "InvalidArgument" },
    );
  }
  const { options } = setup();
  const refund = await refundSubmarineSwap(options);
  await assert.rejects(claimReverseSwap({ ...options, previous: refund }), {
    code: "InvalidArgument",
  });
});

test(
  "real WASM chain refund signs once and rebroadcasts identical bytes after a lost response",
  { timeout: 10000 },
  async () => {
    await init();
    const load = async (name) =>
      JSON.parse(
        await readFile(
          new URL(`../../tests/fixtures/lusdt-v1/${name}`, import.meta.url),
          "utf8",
        ),
      );
    const wire = await load("wire-contract.json");
    const vectors = await load("liquid-golden-vectors.json");
    const response = wire.create.chain.response;
    const address = response.lockupDetails.lockupAddress;
    const requests = [];
    const bodies = [];
    let txid;
    const server = createServer(async (req, res) => {
      requests.push(req.url);
      if (req.url === "/tx") {
        let hex = "";
        for await (const chunk of req) hex += chunk;
        bodies.push(hex);
        if (bodies.length === 1)
          req.socket.destroy(); // Accepted, response lost.
        else res.end(txid);
        return;
      }
      assert.equal(req.url, `/address/${address}/txs`);
      res.setHeader("content-type", "application/json");
      res.end(
        JSON.stringify([
          {
            txid: "11".repeat(32),
            vin: [],
            status: { confirmed: true },
            vout: [
              {
                value: 100000,
                scriptpubkey_address: address,
                scriptpubkey: `5120${vectors.cases.bitcoinChainUserLock.outputKey}`,
              },
            ],
          },
        ]),
      );
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const url = `http://127.0.0.1:${server.address().port}`;
    const script = SwapScript.fromChain(
      "bitcoin",
      "regtest",
      "lockup",
      response.lockupDetails,
      vectors.inputs.refundPublicKey,
    );
    let signed;
    try {
      const options = {
        status: "transaction.confirmed",
        tipHeight: 123,
        createResponse: response,
        script,
        params: {
          swapId: response.id,
          outputAddress: address,
          keysSecretHex: vectors.inputs.refundPrivateKey,
          makerBaseUrl: `${url}/v2`,
          network: "regtest",
          bitcoinEsploraUrl: url,
          feeAbsoluteSat: 50,
        },
      };
      signed = await refundChainSwap(options);
      assert.equal(signed.kind, "signed");
      assert.equal(signed.path, "script");
      txid = signed.transaction.txid();
      const hex = signed.transaction.hex();
      assert.match(hex, /^020000000001/); // A transaction with a witness.
      assert.ok(hex.endsWith("7b000000"), "must retain CLTV height 123");
      await assert.rejects(signed.transaction.broadcast("regtest", url));
      const retry = await refundChainSwap({
        ...options,
        previous: signed,
        params: { ...options.params, feeAbsoluteSat: 1000 },
      });
      assert.equal(retry.transaction, signed.transaction);
      assert.equal(await retry.transaction.broadcast("regtest", url), txid);
      assert.deepEqual(bodies, [hex, hex]);
      assert.deepEqual(requests, [`/address/${address}/txs`, "/tx", "/tx"]);
    } finally {
      signed?.transaction.free();
      script.free();
      await new Promise((resolve) => server.close(resolve));
    }
  },
);
