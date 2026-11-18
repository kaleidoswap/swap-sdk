// Submarine swaps against a real maker: on-chain BTC or L-BTC in, a Lightning
// invoice paid out, or the lockup refunded.
import assert from "node:assert/strict";
import { before, describe, test } from "node:test";

import { SwapScript } from "../../dist/index.node.js";
import {
  NETWORK,
  freshMaster,
  live,
  pairCard,
  mineTo,
  settled,
  skip,
  until,
  waitStatus,
} from "./harness.mjs";

const CHAINS = [
  { from: "BTC", kind: "bitcoin", node: "btc", esplora: "bitcoinEsploraUrl" },
  {
    from: "L-BTC",
    kind: "liquid",
    node: "liquid",
    esplora: "liquidEsploraUrl",
  },
];

const FAILED = [
  "invoice.failedToPay",
  "transaction.lockupFailed",
  "swap.expired",
];

describe("submarine", { skip }, () => {
  let env;
  let master;
  let index = 0n;

  before(async () => {
    env = await live();
    master = freshMaster();
  });

  /** A submarine swap from `chain` paying `invoice`, with its refund key. */
  async function create(chain, invoice) {
    const card = await pairCard(
      () => env.client.submarinePairs(),
      chain.from,
      "BTC",
    );
    const key = master.deriveSwapKey(index++);
    const response = await env.client.createSubmarineSwap(NETWORK, {
      from: chain.from,
      to: "BTC",
      invoice,
      refundPublicKey: key.publicKey,
      pairHash: card.hash,
    });
    const script = SwapScript.fromSubmarine(
      chain.kind,
      NETWORK,
      response,
      key.publicKey,
    );
    return { response, script, key, node: env[chain.node] };
  }

  function txParams(chain, swap, outputAddress, extra = {}) {
    return {
      outputAddress,
      swapId: swap.response.id,
      keysSecretHex: swap.key.secretKey,
      makerBaseUrl: env.makerUrl,
      network: NETWORK,
      [chain.esplora]:
        chain.kind === "bitcoin" ? env.btcEsplora : env.liquidEsplora,
      feeSatPerVb: 1,
      ...extra,
    };
  }

  /** Broadcasts `tx` and waits until the chain's node has it. */
  async function broadcast(chain, swap, tx) {
    const txid = await tx.broadcast(
      NETWORK,
      chain.kind === "bitcoin" ? env.btcEsplora : undefined,
      chain.kind === "liquid" ? env.liquidEsplora : undefined,
    );
    assert.equal(txid, tx.txid());
    await until(`${txid} on ${chain.from}`, () => swap.node.knows(txid));
    return txid;
  }

  /**
   * A swap funded with half its expected amount, confirmed. The maker never
   * pays out an underpaid submarine; the lockup is the taker's to refund.
   */
  async function underpaid(chain) {
    const { invoice } = await env.ln.invoice(60_000);
    const swap = await create(chain, invoice);
    const txid = await swap.node.send(
      swap.response.address,
      swap.response.expectedAmount / 2n,
    );
    await swap.node.mine(1);
    const esplora =
      chain.kind === "bitcoin" ? env.btcEsplora : env.liquidEsplora;
    await until(`lockup ${txid} in esplora`, async () => {
      const response = await fetch(`${esplora}/tx/${txid}/status`);
      return response.ok && (await response.json()).confirmed;
    });
    return swap;
  }

  for (const chain of CHAINS) {
    test(`${chain.from}: the maker pays the invoice and claims the lockup`, async () => {
      const { invoice, paymentHash } = await env.ln.invoice(60_000);
      const swap = await create(chain, invoice);
      assert.equal(swap.response.expectedAmount > 60_000n, true);

      await swap.node.send(swap.response.address, swap.response.expectedAmount);
      await swap.node.mine(1);
      await waitStatus(env.client, swap.response.id, "transaction.claimed", {
        mine: swap.node,
        failOn: FAILED,
      });
      const preimage = await env.client.submarinePreimage(swap.response.id);
      assert.equal(typeof preimage.preimage, "string");
      assert.ok(paymentHash.length === 64);
    });

    test(`${chain.from}: an underpaid lockup is refunded after the timelock`, async () => {
      const swap = await underpaid(chain);
      const params = txParams(chain, swap, await swap.node.newAddress(), {
        cooperative: false,
      });
      // Before the timeout the chain refuses the refund as non-final.
      await assert.rejects(
        swap.script
          .constructRefund(params)
          .then((tx) => broadcast(chain, swap, tx)),
      );

      await mineTo(swap.node, Number(swap.response.timeoutBlockHeight));
      await broadcast(
        chain,
        swap,
        await settled(() => swap.script.constructRefund(params)),
      );
    });
  }

  test("BTC: an underpaid lockup is refunded cooperatively with swapAuth", async () => {
    const chain = CHAINS[0];
    const swap = await underpaid(chain);
    assert.ok(swap.response.swapAuth, "create returns the swap credential");
    const params = txParams(chain, swap, await swap.node.newAddress(), {
      cooperative: true,
      swapAuth: swap.response.swapAuth,
    });
    // No locktime: the maker co-signs at once.
    await broadcast(
      chain,
      swap,
      await settled(() => swap.script.constructRefund(params)),
    );
  });

  test("BTC: a recovered swapAuth authorizes the cooperative refund", async () => {
    const chain = CHAINS[0];
    const swap = await underpaid(chain);
    // The create response is lost; the swap key is all the wallet has.
    const swapAuth = await env.client.recoverSwapAuth(
      swap.response.id,
      swap.key.secretKey,
    );
    assert.equal(typeof swapAuth, "string");
    const params = txParams(chain, swap, await swap.node.newAddress(), {
      cooperative: true,
      swapAuth,
    });
    await broadcast(
      chain,
      swap,
      await settled(() => swap.script.constructRefund(params)),
    );
  });

  test("L-BTC: the maker co-signs no submarine refund; the timelock is the way out", async () => {
    const chain = CHAINS[1];
    const swap = await underpaid(chain);
    await assert.rejects(
      swap.script.constructRefund(
        txParams(chain, swap, await swap.node.newAddress(), {
          cooperative: true,
          swapAuth: swap.response.swapAuth,
        }),
      ),
      /unsupported_cooperative|Bitcoin L1/,
    );
  });
});
