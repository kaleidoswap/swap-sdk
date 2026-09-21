/**
 * Submarine swap: on-chain BTC -> a Lightning invoice. You lock bitcoin, the
 * maker pays the invoice and claims the lockup. The SDK owns no wallet, so it
 * prints what to fund and stops.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" BOLT11_INVOICE="lntbs…" \
 *     node examples/02-submarine.ts [--refund]
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";

import {
  BoltzClient,
  init,
  SwapMasterKey,
  SwapScript,
  toJson,
} from "@kaleidorg/swap-sdk";

import {
  SIGNET_BITCOIN_ESPLORA,
  die,
  makerUrl,
  network,
  required,
  swapIndex,
  waitForStatus,
} from "./common.ts";

const STATE_DIR = new URL("./swaps/", import.meta.url);

async function persist(id: string, record: unknown): Promise<void> {
  await mkdir(STATE_DIR, { recursive: true });
  // `toJson`, not `JSON.stringify`: the response carries bigint amounts, and a
  // throw here would land on the one step whose purpose is keeping the refund.
  await writeFile(new URL(`${id}.json`, STATE_DIR), toJson(record, 2));
}

try {
  await init();

  const net = network();
  const client = BoltzClient.forNetwork(net);
  const index = swapIndex();
  const master = SwapMasterKey.fromSwapMnemonic(
    required(
      "KALEIDO_SWAP_MNEMONIC",
      "the persisted 12-word swap mnemonic, not a disposable one",
    ),
    net,
  );
  const refundKey = master.deriveSwapKey(index);

  if (process.argv.includes("--refund")) {
    const swapId = required("KALEIDO_SWAP_ID", "the id of the swap to refund");
    const saved = JSON.parse(
      await readFile(new URL(`${swapId}.json`, STATE_DIR), "utf8"),
    );
    const script = SwapScript.fromSubmarine(
      "bitcoin",
      net,
      saved.response,
      refundKey.publicKey,
    );
    const refund = await script.constructRefund({
      outputAddress: required(
        "REFUND_ADDRESS",
        "where the refunded bitcoin should land",
      ),
      swapId,
      keysSecretHex: refundKey.secretKey,
      boltzBaseUrl: saved.makerUrl,
      network: net,
      bitcoinEsploraUrl: SIGNET_BITCOIN_ESPLORA,
      feeSatPerVb: 2,
    });
    // Cooperative by default: the maker co-signs, so no waiting out the timeout.
    console.log(
      `refund broadcast: ${await refund.broadcast(net, SIGNET_BITCOIN_ESPLORA)}`,
    );
    process.exit(0);
  }

  const invoice = required(
    "BOLT11_INVOICE",
    "the Lightning invoice the maker should pay",
  );

  // Passing the card's hash back is what holds the maker to the terms shown.
  const pairs = await client.submarinePairs();
  const card = pairs?.BTC?.BTC;
  if (!card)
    throw new Error(
      "the maker does not advertise the BTC -> BTC submarine pair",
    );

  const response = await client.createSubmarineSwap(net, {
    from: "BTC",
    to: "BTC",
    invoice,
    refundPublicKey: refundKey.publicKey,
    pairHash: card.hash,
  });

  // Re-derives the lockup against our own refund key: a tree we could not
  // refund has to fail here, before funding, not after.
  SwapScript.fromSubmarine("bitcoin", net, response, refundKey.publicKey);

  await persist(response.id, {
    swapIndex: index,
    makerUrl: makerUrl(net),
    response,
  });

  console.log(`swap id:      ${response.id}`);
  console.log(`swap index:   ${index}  (never reuse it)`);
  console.log(`saved to:     examples/swaps/${response.id}.json`);
  console.log(
    `\nFund exactly ${response.expectedAmount} sat to:\n  ${response.address}\n`,
  );
  if (card.limits.maximalZeroConf === 0n) {
    console.log(
      "This pair accepts no zero-conf: the maker waits for a confirmation.",
    );
  }
  console.log("Waiting — the maker pays the invoice once the lockup lands.\n");

  await waitForStatus(client, response.id, "transaction.claimed");
  console.log("\nDone: the Lightning invoice was paid and the lockup claimed.");
} catch (error) {
  die(error);
}
