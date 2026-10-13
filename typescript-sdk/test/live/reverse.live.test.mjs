// Reverse swaps against a real maker: a Lightning payment in, the maker's
// L-BTC lockup claimed out, watched over the WebSocket, then found again from
// the master key alone.
import assert from "node:assert/strict";
import { before, describe, test } from "node:test";

import { SwapScript, SwapWsApi } from "../../dist/index.node.js";
import {
  NETWORK,
  freshMaster,
  live,
  settled,
  skip,
  until,
  waitStatus,
} from "./harness.mjs";

const LBTC = { to: "L-BTC", kind: "liquid", node: "liquid" };
const BTC = { to: "BTC", kind: "bitcoin", node: "btc" };

const FAILED = ["invoice.expired", "transaction.failed", "swap.expired"];

describe("reverse", { skip }, () => {
  let env;
  let master;
  let index = 0n;
  const created = [];

  before(async () => {
    env = await live();
    master = freshMaster();
  });

  async function card(chain) {
    return (await env.client.reversePairs())?.BTC?.[chain.to];
  }

  /** A Lightning -> `chain` reverse swap, its invoice paid. */
  async function paidReverse(chain, amount = 60_000n) {
    const pair = await card(chain);
    assert.ok(pair, `the maker serves no BTC -> ${chain.to} reverse pair`);
    const at = index++;
    const key = master.deriveSwapKey(at);
    const preimage = master.derivePreimage(at);
    const response = await env.client.createReverseSwap(NETWORK, {
      from: "BTC",
      to: chain.to,
      invoiceAmount: amount,
      preimageHash: preimage.sha256,
      claimPublicKey: key.publicKey,
      pairHash: pair.hash,
    });
    created.push({ id: response.id, index: at });
    const script = SwapScript.fromReverse(
      chain.kind,
      NETWORK,
      response,
      key.publicKey,
    );
    // Held by the maker until the claim reveals the preimage.
    await env.ln.pay(response.invoice);
    return { response, script, key, preimage, chain, node: env[chain.node] };
  }

  /** The claim transaction, built once the maker's lockup has confirmed. */
  function buildClaim(swap, outputAddress, cooperative) {
    const esplora =
      swap.chain.kind === "bitcoin"
        ? { bitcoinEsploraUrl: env.btcEsplora }
        : { liquidEsploraUrl: env.liquidEsplora };
    return swap.script.constructClaim(swap.preimage.preimage, {
      outputAddress,
      swapId: swap.response.id,
      keysSecretHex: swap.key.secretKey,
      makerBaseUrl: env.makerUrl,
      network: NETWORK,
      feeSatPerVb: 1,
      cooperative,
      ...esplora,
    });
  }

  async function confirmed(swap) {
    await waitStatus(
      env.client,
      swap.response.id,
      ["transaction.mempool", "transaction.confirmed"],
      { failOn: FAILED },
    );
    await waitStatus(env.client, swap.response.id, "transaction.confirmed", {
      mine: swap.node,
      failOn: FAILED,
    });
  }

  async function claim(swap, cooperative) {
    await confirmed(swap);
    const outputAddress = await swap.node.newAddress();
    const tx = await settled(() =>
      buildClaim(swap, outputAddress, cooperative),
    );
    const txid = await tx.broadcast(
      NETWORK,
      swap.chain.kind === "bitcoin" ? env.btcEsplora : undefined,
      swap.chain.kind === "liquid" ? env.liquidEsplora : undefined,
    );
    assert.equal(txid, tx.txid());
    await until(`claim ${txid}`, () => swap.node.knows(txid));
    await swap.node.mine(1);
    await waitStatus(env.client, swap.response.id, "invoice.settled", {
      failOn: FAILED,
    });
    return tx;
  }

  test("L-BTC: a script-path claim settles the invoice", async () => {
    const swap = await paidReverse(LBTC);
    await claim(swap, false);
  });

  test("L-BTC: the maker co-signs no reverse claim", async () => {
    const swap = await paidReverse(LBTC);
    await confirmed(swap);
    const outputAddress = await env.liquid.newAddress();
    await assert.rejects(
      until("the maker's answer", async () => {
        try {
          await buildClaim(swap, outputAddress, true);
          return "built";
        } catch (error) {
          // A lockup still reaching the index is not the answer.
          return /unsupported_cooperative/.test(error.message)
            ? Promise.reject(error)
            : undefined;
        }
      }),
      /unsupported_cooperative/,
    );
    // The swap is still claimable by script path.
    await claim(swap, false);
  });

  test("BTC: a cooperative claim settles the invoice", async (t) => {
    if (!(await card(BTC))) {
      t.skip("the maker publishes no BTC -> BTC reverse pair");
      return;
    }
    const swap = await paidReverse(BTC);
    await claim(swap, true);
  });

  test("the WebSocket reports a reverse swap through to invoice.settled", async () => {
    const ws = new SwapWsApi(env.wsUrl);
    const loop = ws.runWsLoop().catch(() => undefined);
    const updates = ws.updates();
    const seen = [];
    try {
      await until("the WebSocket to connect", () => ws.isConnected(), {
        timeoutMs: 20_000,
      });
      const swap = await paidReverse(LBTC);
      await ws.subscribeSwap(swap.response.id);
      // One reader drains the cursor; a second next() would steal updates.
      const all = [];
      let reading = true;
      const reader = (async () => {
        while (reading) {
          try {
            all.push(await updates.next());
          } catch {
            return;
          }
        }
      })();
      const claiming = claim(swap, false);
      await until(
        () =>
          `invoice.settled over the WebSocket (saw ${JSON.stringify(all.map((u) => [u?.id, u?.status]))})`,
        () => {
          for (const update of all.splice(0)) {
            if (
              update?.id === swap.response.id &&
              seen.at(-1) !== update.status
            ) {
              seen.push(update.status);
            }
          }
          return seen.includes("invoice.settled");
        },
        { timeoutMs: 120_000, everyMs: 500 },
      );
      reading = false;
      void reader;
      await claiming;
      assert.ok(
        seen.includes("transaction.mempool") ||
          seen.includes("transaction.confirmed"),
        `saw ${seen.join(" -> ")}`,
      );
    } finally {
      // Without close() the socket keeps this test file running forever.
      await ws.close();
      await loop;
    }
  });

  test("the master key alone finds every swap and the next free index", async () => {
    assert.ok(created.length > 0, "run with the swaps above");
    const restored = await master.restore(env.client);
    const ids = new Set(restored.map((swap) => swap.id));
    for (const { id } of created)
      assert.ok(ids.has(id), `restore misses ${id}`);

    const highest = await master.restoreIndex(env.client);
    assert.equal(
      BigInt(highest.index ?? highest),
      BigInt(Math.max(...created.map((swap) => Number(swap.index)))),
    );
  });
});
