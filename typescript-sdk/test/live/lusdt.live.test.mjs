// L-USDT against a real maker: the caller-funded PSET spends. The SDK builds
// a template; the elementsd wallet adds the L-BTC that pays the fee and signs
// it; the SDK checks the result and signs the swap input.
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
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
import { fundWithExplicitInput } from "./pset.mjs";

const FEE = 1_000n;
const ZERO = "00".repeat(32);
const FAILED = ["invoice.expired", "transaction.failed", "swap.expired"];

describe("L-USDT", { skip }, () => {
  let env;
  let master;
  let index = 0n;

  before(async () => {
    env = await live();
    master = freshMaster();
  });

  function psetParams(swapId, outputAddress) {
    return {
      outputAddress,
      swapId,
      maxFee: FEE,
      quotedFeeCap: FEE,
      makerBaseUrl: env.makerUrl,
      network: NETWORK,
      liquidEsploraUrl: env.liquidEsplora,
    };
  }

  /** What a Liquid wallet returns for `prepared`: funded, its input signed. */
  async function fund(prepared) {
    const template = prepared.template();
    const utxo = await env.liquid.explicitUtxo(100_000n);
    const change = await env.liquid.explicitAddress();
    const pset = fundWithExplicitInput(template.pset, {
      policyAssetHex: template.policyAssetId,
      utxo,
      changeScriptHex: change.scriptHex,
      fee: FEE,
    });
    const signed = await env.liquid.rpc(
      "walletprocesspsbt",
      pset,
      true,
      "ALL",
      true,
      true,
    );
    return {
      pset: signed.psbt,
      // The payout is explicit: nothing to unblind.
      paymentOutputSecrets: {
        assetId: template.swapAssetId,
        value: template.amount,
        assetBlindingFactor: ZERO,
        valueBlindingFactor: ZERO,
      },
    };
  }

  async function broadcast(tx) {
    const txid = await tx.broadcast(NETWORK, undefined, env.liquidEsplora);
    assert.equal(txid, tx.txid());
    await until(`${txid} on Liquid`, () => env.liquid.knows(txid));
    return txid;
  }

  test("reverse: a funded PSET claims the maker's L-USDT lockup", async () => {
    const pair = await pairCard(
      () => env.client.reversePairs(),
      "BTC",
      "L-USDT",
    );
    const at = index++;
    const key = master.deriveSwapKey(at);
    const preimage = master.derivePreimage(at);
    const response = await env.client.createReverseSwap(NETWORK, {
      from: "BTC",
      to: "L-USDT",
      invoiceAmount: 60_000n,
      preimageHash: preimage.sha256,
      claimPublicKey: key.publicKey,
      pairHash: pair.hash,
    });
    const script = SwapScript.fromReverse(
      "liquid",
      NETWORK,
      response,
      key.publicKey,
    );
    await env.ln.pay(response.invoice);
    await waitStatus(env.client, response.id, "transaction.confirmed", {
      mine: env.liquid,
      failOn: FAILED,
    });

    const payout = await env.liquid.explicitAddress();
    const prepared = await settled(() =>
      script.prepareLiquidClaim(psetParams(response.id, payout.address)),
    );
    const template = prepared.template();
    assert.equal(template.amount, BigInt(response.onchainAmount));

    // Without L-BTC for the fee the SDK refuses to finalize.
    assert.throws(
      () =>
        prepared.finalizeClaim(
          {
            pset: template.pset,
            paymentOutputSecrets: {
              assetId: template.swapAssetId,
              value: template.amount,
              assetBlindingFactor: ZERO,
              valueBlindingFactor: ZERO,
            },
          },
          key.secretKey,
          preimage.preimage,
        ),
      /fee|policy/i,
    );

    const claim = prepared.finalizeClaim(
      await fund(prepared),
      key.secretKey,
      preimage.preimage,
    );
    await broadcast(claim);
    await env.liquid.mine(1);
    await waitStatus(env.client, response.id, "invoice.settled", {
      failOn: FAILED,
    });
  });

  /** An L-USDT -> Lightning submarine swap paying a fresh invoice. */
  async function submarine(invoice) {
    const card = await pairCard(
      () => env.client.submarinePairs(),
      "L-USDT",
      "BTC",
    );
    const key = master.deriveSwapKey(index++);
    invoice ??= (await env.ln.invoice(60_000)).invoice;
    const response = await env.client.createSubmarineSwap(NETWORK, {
      from: "L-USDT",
      to: "BTC",
      invoice,
      refundPublicKey: key.publicKey,
      pairHash: card.hash,
    });
    const script = SwapScript.fromSubmarine(
      "liquid",
      NETWORK,
      response,
      key.publicKey,
    );
    return { response, script, key, asset: card.fromAssetId };
  }

  test("submarine: the maker pays the invoice and claims the L-USDT lockup", async () => {
    const swap = await submarine();
    await env.liquid.send(
      swap.response.address,
      swap.response.expectedAmount,
      swap.asset,
    );
    await env.liquid.mine(1);
    await waitStatus(env.client, swap.response.id, "transaction.claimed", {
      mine: env.liquid,
      failOn: [
        "invoice.failedToPay",
        "transaction.lockupFailed",
        "swap.expired",
      ],
    });
  });

  test("submarine: a funded PSET refunds the L-USDT lockup after a failed payment", async () => {
    // The receiver holds the maker's HTLC and then rejects it.
    const paymentHash = createHash("sha256")
      .update(randomBytes(32))
      .digest("hex");
    const swap = await submarine(
      await env.ln.invoiceForHash(60_000, paymentHash),
    );
    const lockup = await env.liquid.send(
      swap.response.address,
      swap.response.expectedAmount,
      swap.asset,
    );
    await env.liquid.mine(1);
    // The maker's node retries; every attempt arrives held and is rejected,
    // until the maker gives up on the payout.
    await until(
      "the maker to give up on the payout",
      async () => {
        await env.ln.fail(paymentHash).catch(() => undefined);
        const { status } = await env.client.swap(swap.response.id);
        assert.notEqual(status, "transaction.claimed");
        return status === "invoice.failedToPay";
      },
      { timeoutMs: 600_000, everyMs: 3_000 },
    );

    const payout = await env.liquid.explicitAddress();
    const prepared = await settled(() =>
      swap.script.prepareLiquidRefund({
        ...psetParams(swap.response.id, payout.address),
      }),
    );
    assert.equal(prepared.template().swapAssetId, swap.asset);
    const refund = prepared.finalizeRefund(
      await fund(prepared),
      swap.key.secretKey,
    );
    assert.ok(
      refund
        .hex()
        .includes(Buffer.from(lockup, "hex").reverse().toString("hex")),
      "the refund spends the lockup",
    );
    // Before the timeout Elements refuses it as non-final.
    await assert.rejects(broadcast(refund), /non-final|locktime/i);

    await mineTo(env.liquid, Number(swap.response.timeoutBlockHeight));
    await broadcast(refund);
  });

  test("submarine: an underpaid L-USDT lockup cannot be refunded through the SDK", async () => {
    // Pins current behaviour: caller-funded PSET spends require the exact
    // expected amount, refunds included.
    const swap = await submarine();
    const lockup = await env.liquid.send(
      swap.response.address,
      swap.response.expectedAmount / 2n,
      swap.asset,
    );
    await env.liquid.mine(1);
    await until(`lockup ${lockup} in the node`, () => env.liquid.knows(lockup));
    await assert.rejects(
      swap.script.prepareLiquidRefund({
        ...psetParams(
          swap.response.id,
          (await env.liquid.explicitAddress()).address,
        ),
        lockupTxHex: await env.liquid.rawTransaction(lockup),
      }),
      /amount mismatch/,
    );
  });
  test("chain: BTC in, the maker's L-USDT lockup claimed with a funded PSET", async () => {
    const pair = await pairCard(() => env.client.chainPairs(), "BTC", "L-USDT");
    const at = index;
    index += 2n;
    const claimKey = master.deriveSwapKey(at);
    const refundKey = master.deriveSwapKey(at + 1n);
    const preimage = master.derivePreimage(at);
    const response = await env.client.createChainSwap(NETWORK, {
      from: "BTC",
      to: "L-USDT",
      preimageHash: preimage.sha256,
      claimPublicKey: claimKey.publicKey,
      refundPublicKey: refundKey.publicKey,
      userLockAmount: 60_000n,
      pairHash: pair.hash,
    });
    // Both legs rebuild from the response, or the swap is refused here.
    SwapScript.fromChain(
      "bitcoin",
      NETWORK,
      "lockup",
      response.lockupDetails,
      refundKey.publicKey,
    );
    const script = SwapScript.fromChain(
      "liquid",
      NETWORK,
      "claim",
      response.claimDetails,
      claimKey.publicKey,
    );

    await env.btc.send(
      response.lockupDetails.lockupAddress,
      response.lockupDetails.amount,
    );
    await env.btc.mine(1);
    await waitStatus(
      env.client,
      response.id,
      ["transaction.server.mempool", "transaction.server.confirmed"],
      { mine: env.btc, failOn: [...FAILED, "transaction.lockupFailed"] },
    );
    await waitStatus(env.client, response.id, "transaction.server.confirmed", {
      mine: env.liquid,
      failOn: FAILED,
    });

    const payout = await env.liquid.explicitAddress();
    const prepared = await settled(() =>
      script.prepareLiquidClaim(psetParams(response.id, payout.address)),
    );
    assert.equal(
      prepared.template().amount,
      BigInt(response.claimDetails.amount),
    );
    const claim = prepared.finalizeClaim(
      await fund(prepared),
      claimKey.secretKey,
      preimage.preimage,
    );
    await broadcast(claim);
    await env.liquid.mine(1);
    await waitStatus(env.client, response.id, "transaction.claimed", {
      mine: env.btc,
      failOn: FAILED,
    });
  });
});
