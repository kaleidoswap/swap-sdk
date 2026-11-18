// A partner `kld_` key against a real maker: the keyed pair cards, one
// submarine and one reverse swap created and settled through the keyed
// client, the partner fee in the amounts and in a `KaleidoSwap` quote, and the
// maker's `X-Kaleido-Attribution` answer.
//
// Runs only with KALEIDO_LIVE_API_KEY set (test/live/README.md). Set
// KALEIDO_LIVE_PARTNER_FEE_PCT too to pin the key's partner fee, in percent.
import assert from "node:assert/strict";
import { before, describe, test } from "node:test";

import { MemoryStore } from "../../dist/kaleidoswap.js";
import {
  SwapScript,
  createKaleidoSwap,
  mnemonicSigner,
} from "../../dist/index.node.js";
import {
  NETWORK,
  freshMaster,
  live,
  pairCard,
  settled,
  skip as liveSkip,
  until,
  waitStatus,
} from "./harness.mjs";

const skip =
  liveSkip ||
  (process.env.KALEIDO_LIVE_API_KEY
    ? false
    : "partner live tests: set KALEIDO_LIVE_API_KEY (test/live/README.md)");
const PINNED_PCT =
  process.env.KALEIDO_LIVE_PARTNER_FEE_PCT === undefined
    ? undefined
    : Number(process.env.KALEIDO_LIVE_PARTNER_FEE_PCT);
const AMOUNT = 60_000;

/** The partner fee the maker charges on `amount`: floor(amount × E). */
const partnerFee = (amount, pct) =>
  BigInt(Math.floor((amount * Math.round(pct * 10_000)) / 1_000_000));

describe("partner key", { skip }, () => {
  let env;
  let master;
  let index = 0n;

  before(async () => {
    env = await live();
    master = freshMaster();
  });

  /** The keyed and anonymous cards for one route, checked against each other. */
  async function cards(list, from, to) {
    const anon = await pairCard(() => env.client[list](), from, to);
    const keyed = await pairCard(() => env.keyedClient[list](), from, to);
    assert.equal(env.keyedClient.lastAttribution, "api_key");
    const pct = keyed.fees.partnerPercentage ?? 0;
    if (PINNED_PCT !== undefined) assert.equal(pct, PINNED_PCT);
    assert.ok(
      Math.abs(keyed.fees.percentage - (anon.fees.percentage + pct)) < 1e-9,
      `keyed ${keyed.fees.percentage}% is not ${anon.fees.percentage}% + ${pct}%`,
    );
    if (pct > 0) assert.notEqual(keyed.hash, anon.hash);
    return { anon, keyed, pct };
  }

  test("submarine: the keyed card prices, creates and settles", async () => {
    // Whichever on-chain side the maker publishes a submarine route from.
    const listed = (await env.client.submarinePairs()) ?? {};
    const chain = listed.BTC?.BTC
      ? { from: "BTC", kind: "bitcoin", node: env.btc }
      : { from: "L-BTC", kind: "liquid", node: env.liquid };
    const { anon, keyed, pct } = await cards(
      "submarinePairs",
      chain.from,
      "BTC",
    );
    const create = async (client, card) => {
      const { invoice } = await env.ln.invoice(AMOUNT);
      const key = master.deriveSwapKey(index++);
      const response = await client.createSubmarineSwap(NETWORK, {
        from: chain.from,
        to: "BTC",
        invoice,
        refundPublicKey: key.publicKey,
        pairHash: card.hash,
      });
      return { response, key };
    };
    const swap = await create(env.keyedClient, keyed);
    assert.equal(env.keyedClient.lastAttribution, "api_key");
    // An anonymous create of the same invoice amount, never funded: the
    // difference is the partner fee, charged on the invoice like the swap fee.
    const reference = await create(env.client, anon);
    assert.equal(
      swap.response.expectedAmount - reference.response.expectedAmount,
      partnerFee(AMOUNT, pct),
    );

    SwapScript.fromSubmarine(
      chain.kind,
      NETWORK,
      swap.response,
      swap.key.publicKey,
    );
    await chain.node.send(swap.response.address, swap.response.expectedAmount);
    await chain.node.mine(1);
    await waitStatus(env.client, swap.response.id, "transaction.claimed", {
      mine: chain.node,
      failOn: [
        "invoice.failedToPay",
        "transaction.lockupFailed",
        "swap.expired",
      ],
    });
  });

  test("reverse L-BTC: fees.partner, the keyed create, and a claimed lockup", async () => {
    const { anon, keyed, pct } = await cards("reversePairs", "BTC", "L-BTC");

    // The high-level quote shows the partner fee as its own line.
    const kaleido = await createKaleidoSwap({
      network: NETWORK,
      store: new MemoryStore(),
      signer: mnemonicSigner(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        NETWORK,
      ),
      makerUrl: env.makerUrl,
      apiKey: process.env.KALEIDO_LIVE_API_KEY,
      bitcoinEsploraUrl: env.btcEsplora,
      liquidEsploraUrl: env.liquidEsplora,
    });
    const quote = await kaleido.quote({
      pay: "lightning:BTC",
      receive: "liquid:BTC",
      amount: { pay: AMOUNT },
    });
    assert.equal(quote.kind, "quote");
    assert.equal(quote.fees.partner, partnerFee(AMOUNT, pct));
    assert.equal(
      quote.fromAmount - quote.toAmount,
      quote.fees.maker + quote.fees.partner + quote.fees.miner,
    );
    assert.equal(kaleido.lastAttribution, "api_key");

    const create = async (client, card) => {
      const at = index++;
      const key = master.deriveSwapKey(at);
      const preimage = master.derivePreimage(at);
      const response = await client.createReverseSwap(NETWORK, {
        from: "BTC",
        to: "L-BTC",
        invoiceAmount: AMOUNT,
        preimageHash: preimage.sha256,
        claimPublicKey: key.publicKey,
        pairHash: card.hash,
      });
      return { response, key, preimage };
    };
    const swap = await create(env.keyedClient, keyed);
    assert.equal(env.keyedClient.lastAttribution, "api_key");
    // An anonymous create of the same amount, never paid: the maker locks up
    // less for the keyed swap by exactly the partner fee.
    const reference = await create(env.client, anon);
    assert.equal(
      reference.response.onchainAmount - swap.response.onchainAmount,
      quote.fees.partner,
    );

    const script = SwapScript.fromReverse(
      "liquid",
      NETWORK,
      swap.response,
      swap.key.publicKey,
    );
    await env.ln.pay(swap.response.invoice);
    const failOn = ["invoice.expired", "transaction.failed", "swap.expired"];
    await waitStatus(
      env.client,
      swap.response.id,
      ["transaction.mempool", "transaction.confirmed"],
      { failOn },
    );
    await waitStatus(env.client, swap.response.id, "transaction.confirmed", {
      mine: env.liquid,
      failOn,
    });
    const outputAddress = await env.liquid.newAddress();
    const tx = await settled(() =>
      script.constructClaim(swap.preimage.preimage, {
        outputAddress,
        swapId: swap.response.id,
        keysSecretHex: swap.key.secretKey,
        makerBaseUrl: env.makerUrl,
        network: NETWORK,
        feeSatPerVb: 1,
        cooperative: false,
        liquidEsploraUrl: env.liquidEsplora,
      }),
    );
    const txid = await tx.broadcast(NETWORK, undefined, env.liquidEsplora);
    await until(`claim ${txid}`, () => env.liquid.knows(txid));
    await env.liquid.mine(1);
    await waitStatus(env.client, swap.response.id, "invoice.settled", {
      failOn,
    });
  });
});
