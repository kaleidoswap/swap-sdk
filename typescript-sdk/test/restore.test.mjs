import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import test from "node:test";
import { init, SwapClient, SwapScript } from "../dist/index.node.js";
import { SwapScript as BrowserSwapScript } from "../dist/index.js";
import { SwapScript as WasmSwapScript } from "../vendor/bindings_wasm.js";

await init();
const readFixture = async (name) =>
  JSON.parse(
    await readFile(
      new URL(
        `../../tests/fixtures/${name}/wire-contract.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  );
const [bitcoin, liquid] = await Promise.all([
  readFixture("rgb-v1"),
  readFixture("lusdt-v1"),
]);

// Reuse the captured script/key vectors as plain BTC / L-BTC swaps: asset
// metadata is irrelevant to the Taproot tree, and is deliberately removed.
function plainVector(fixture, kind, asset) {
  const vector = structuredClone(fixture.create[kind]);
  if (kind === "chain") {
    vector.request = vector.userAmountRequest;
    vector.request.to = asset;
    delete vector.response.claimDetails.assetId;
    delete vector.response.claimDetails.feeAssetId;
  } else {
    vector.request[kind === "submarine" ? "from" : "to"] = asset;
    delete vector.response.rgb;
    delete vector.response.assetId;
    delete vector.response.feeAssetId;
  }
  return { ...vector, kind, asset };
}
const vectors = [
  plainVector(bitcoin, "submarine", "BTC"),
  plainVector(bitcoin, "reverse", "BTC"),
  plainVector(liquid, "submarine", "L-BTC"),
  plainVector(liquid, "reverse", "L-BTC"),
  plainVector(liquid, "chain", "L-BTC"),
];

function restoreEntry(vector, response) {
  const details = (value, claim) => ({
    tree: value.swapTree,
    lockupAddress: value.address ?? value.lockupAddress,
    serverPublicKey:
      value.serverPublicKey ?? value.claimPublicKey ?? value.refundPublicKey,
    timeoutBlockHeight: value.timeoutBlockHeight,
    blindingKey: value.blindingKey,
    amount: value.amount ?? value.expectedAmount ?? value.onchainAmount,
    keyIndex: claim ? 8 : 7,
    ...(claim ? { preimageHash: vector.request.preimageHash } : {}),
  });
  return {
    id: response.id,
    type: vector.kind,
    from: vector.request.from,
    to: vector.request.to,
    status: "swap.created",
    createdAt: 1,
    invoice: vector.request.invoice ?? response.invoice,
    ...(vector.kind === "chain"
      ? {
          refundDetails: details(response.lockupDetails, false),
          claimDetails: details(response.claimDetails, true),
        }
      : vector.kind === "submarine"
        ? { refundDetails: details(response, false) }
        : { claimDetails: details(response, true) }),
  };
}

test("create -> swapRestore -> script supports submarine, reverse and both chain sides", async () => {
  let vector;
  let saved;
  const requests = [];
  const server = createServer(async (req, res) => {
    let text = "";
    for await (const chunk of req) text += chunk;
    requests.push({ path: req.url, body: JSON.parse(text) });
    const restored = req.url === "/swap/restore";
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify(restored ? [saved] : vector.response));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const client = new SwapClient(`http://127.0.0.1:${server.address().port}`);
  try {
    for (vector of vectors) {
      const response = await client[
        {
          submarine: "createSubmarineSwap",
          reverse: "createReverseSwap",
          chain: "createChainSwap",
        }[vector.kind]
      ]("regtest", vector.request);
      saved = restoreEntry(vector, vector.response);
      const [restored] = await client.swapRestore("fixture-xpub", "m");
      // No fake bip21 / acceptZeroConf, or remapping serverPublicKey, is needed.
      assert.equal(restored.bip21, undefined);
      assert.equal(restored.acceptZeroConf, undefined);
      const sides = vector.kind === "chain" ? ["lockup", "claim"] : [undefined];
      for (const side of sides) {
        const claim = vector.kind === "reverse" || side === "claim";
        const key =
          vector.request[claim ? "claimPublicKey" : "refundPublicKey"];
        const chain =
          (claim ? vector.request.to : vector.request.from) === "L-BTC"
            ? "liquid"
            : "bitcoin";
        const original =
          vector.kind === "chain"
            ? SwapScript.fromChain(
                chain,
                "regtest",
                side,
                response[side === "claim" ? "claimDetails" : "lockupDetails"],
                key,
              )
            : SwapScript[claim ? "fromReverse" : "fromSubmarine"](
                chain,
                "regtest",
                response,
                key,
              );
        // Restoration verifies canonical leaves and recomputes the same lockup
        // address. Rust tests additionally compare every reconstructed field.
        const rebuilt = SwapScript.fromRestored(restored, key, "regtest", side);
        assert.ok(original instanceof SwapScript);
        assert.ok(rebuilt instanceof SwapScript);
        const raw = WasmSwapScript.fromRestored(restored, key, "regtest", side);
        raw.free();
      }
    }
    assert.equal(requests.filter((r) => r.path === "/swap/restore").length, 5);
    assert.equal(BrowserSwapScript, SwapScript);
  } finally {
    client.free();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("restoration refuses incomplete, tampered and unsupported recovery data", () => {
  const vector = vectors[4];
  const entry = restoreEntry(vector, vector.response);
  const key = vector.request.claimPublicKey;
  const protocol = (fn, pattern) =>
    assert.throws(fn, (error) => {
      assert.ok(error instanceof Error);
      assert.equal(error.code, "Protocol");
      assert.match(error.message, pattern);
      return true;
    });
  protocol(() => SwapScript.fromRestored(entry, key, "regtest"), /side/);
  for (const asset of ["USDT-RGB", "L-USDT", "ARKD"]) {
    protocol(
      () =>
        SwapScript.fromRestored(
          { ...entry, to: asset },
          key,
          "regtest",
          "claim",
        ),
      /asset context/,
    );
  }
  for (const field of ["claimDetails", "refundDetails"]) {
    const side = field === "claimDetails" ? "claim" : "lockup";
    const sideKey =
      vector.request[
        field === "claimDetails" ? "claimPublicKey" : "refundPublicKey"
      ];
    protocol(
      () =>
        SwapScript.fromRestored(
          { ...entry, [field]: undefined },
          sideKey,
          "regtest",
          side,
        ),
      /missing.*details/,
    );
    const withoutAmount = {
      ...entry,
      [field]: { ...entry[field], amount: undefined },
    };
    if (field === "claimDetails")
      protocol(
        () => SwapScript.fromRestored(withoutAmount, sideKey, "regtest", side),
        /amount/,
      );
    else
      SwapScript.fromRestored(withoutAmount, sideKey, "regtest", side).free();
    const changed = structuredClone(entry);
    changed[field].tree.claimLeaf.output += "51";
    protocol(
      () => SwapScript.fromRestored(changed, sideKey, "regtest", side),
      /non-canonical/,
    );
  }
  const changed = structuredClone(entry);
  changed.claimDetails.preimageHash = "00".repeat(32);
  protocol(
    () => SwapScript.fromRestored(changed, key, "regtest", "claim"),
    /hashlock/,
  );
  assert.throws(
    () => SwapScript.fromRestored(entry, key, "regtest", "other"),
    (error) => error.code === "InvalidArgument" && /side/.test(error.message),
  );
  assert.throws(
    () => SwapScript.fromRestored(entry, {}, "regtest", "claim"),
    (error) =>
      error.code === "InvalidArgument" && /ourPubkeyHex/.test(error.message),
  );
  const submarine = restoreEntry(vectors[0], vectors[0].response);
  protocol(
    () =>
      SwapScript.fromRestored(
        submarine,
        vectors[0].request.refundPublicKey,
        "regtest",
        "lockup",
      ),
    /only valid for a chain/,
  );
});

test("refund recovery accepts missing amount and preserves the authoritative payment hash", () => {
  const vector = vectors[4];
  const entry = restoreEntry(vector, vector.response);
  entry.preimageHash = vector.request.preimageHash;
  entry.invoice = undefined;
  entry.refundDetails.amount = null;
  const key = vector.request.refundPublicKey;
  SwapScript.fromRestored(entry, key, "regtest", "lockup").free();
  assert.throws(
    () =>
      SwapScript.fromRestored(
        { ...entry, preimageHash: "00".repeat(32) },
        key,
        "regtest",
        "lockup",
      ),
    /hashlock/,
  );
  assert.throws(
    () =>
      SwapScript.fromRestored(
        entry,
        vector.request.claimPublicKey,
        "regtest",
        "lockup",
      ),
    /ourPubkey.*refund key.*keyIndex/,
  );
});
