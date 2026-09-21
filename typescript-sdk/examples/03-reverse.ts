/**
 * Reverse swap: a Lightning payment -> L-BTC on Liquid. Settles end to end —
 * the SDK derives the preimage, builds the claim and broadcasts it. The only
 * manual step is paying the printed invoice.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" LIQUID_CLAIM_ADDRESS="tlq1…" \
 *     node examples/03-reverse.ts
 */
import { mkdir, writeFile } from "node:fs/promises";

import {
  SwapClient,
  init,
  SwapMasterKey,
  SwapScript,
  toJson,
} from "@kaleidorg/swap-sdk";

import {
  SIGNET_LIQUID_ESPLORA,
  die,
  makerUrl,
  network,
  optionalInt,
  required,
  swapIndex,
  waitForStatus,
} from "./common.ts";

const STATE_DIR = new URL("./swaps/", import.meta.url);

try {
  await init();

  const net = network();
  const client = SwapClient.forNetwork(net);
  const index = swapIndex();
  const claimAddress = required(
    "LIQUID_CLAIM_ADDRESS",
    "the Liquid address the L-BTC payout should reach",
  );
  // The SDK exposes no Liquid address parser and the claim is the first step
  // that would reject one — by which time the invoice is paid. A prefix check
  // is not validation, but it catches a mainnet address used on signet.
  const expectedPrefixes =
    net === "signet" ? ["tlq1", "tex1", "vjT", "vjU"] : ["el1", "ert1"];
  if (!expectedPrefixes.some((prefix) => claimAddress.startsWith(prefix))) {
    throw new Error(
      `LIQUID_CLAIM_ADDRESS=${claimAddress} does not look like a ${net} Liquid address ` +
        `(expected one of ${expectedPrefixes.join(", ")}). Liquid networks encode addresses ` +
        `similarly enough that a wrong-network payout is unrecoverable.`,
    );
  }

  const liquidEsplora = process.env.LIQUID_ESPLORA_URL ?? SIGNET_LIQUID_ESPLORA;
  const amount = BigInt(optionalInt("INVOICE_AMOUNT_SATS", 50_000));

  const master = SwapMasterKey.fromSwapMnemonic(
    required("KALEIDO_SWAP_MNEMONIC", "the persisted 12-word swap mnemonic"),
    net,
  );
  const claimKey = master.deriveSwapKey(index);
  const preimage = master.derivePreimage(index);

  const pairs = await client.reversePairs();
  const card = pairs?.BTC?.["L-BTC"];
  if (!card)
    throw new Error(
      "the maker does not advertise the BTC -> L-BTC reverse pair",
    );
  if (amount < card.limits.minimal || amount > card.limits.maximal) {
    throw new Error(
      `INVOICE_AMOUNT_SATS=${amount} is outside the pair limits ` +
        `${card.limits.minimal}–${card.limits.maximal} sat`,
    );
  }

  // Amounts cross as bigint: a plain number throws in the glue, uncoded.
  const response = await client.createReverseSwap(net, {
    from: "BTC",
    to: "L-BTC",
    invoiceAmount: amount,
    preimageHash: preimage.sha256,
    claimPublicKey: claimKey.publicKey,
    pairHash: card.hash,
  });

  // Re-derives the lockup against our own claim key, so a tree we could not
  // claim fails here rather than after the invoice is paid.
  const script = SwapScript.fromReverse(
    "liquid",
    net,
    response,
    claimKey.publicKey,
  );

  await mkdir(STATE_DIR, { recursive: true });
  await writeFile(
    new URL(`${response.id}.json`, STATE_DIR),
    // swapAuth rides along here, issued once. Persist it; never log the response.
    toJson({ swapIndex: index, makerUrl: makerUrl(net), response }, 2),
  );

  console.log(`swap id:    ${response.id}`);
  console.log(`swap index: ${index}  (never reuse it)`);
  console.log(`saved to:   examples/swaps/${response.id}.json`);
  console.log(`\nPay this invoice (${amount} sat):\n\n${response.invoice}\n`);
  console.log("Waiting for the maker's Liquid lockup to confirm.\n");

  await waitForStatus(client, response.id, "transaction.confirmed");

  const claim = await script.constructClaim(preimage.preimage, {
    outputAddress: claimAddress,
    swapId: response.id,
    keysSecretHex: claimKey.secretKey,
    makerBaseUrl: makerUrl(net),
    network: net,
    liquidEsploraUrl: liquidEsplora,
  });
  console.log(
    `\nclaim broadcast: ${await claim.broadcast(net, undefined, liquidEsplora)}`,
  );

  // The maker settles the hold invoice once it sees the preimage on chain.
  await waitForStatus(client, response.id, "invoice.settled");
  console.log(
    `\nDone: ${response.onchainAmount} sat of L-BTC claimed to ${claimAddress}`,
  );
} catch (error) {
  die(error);
}
