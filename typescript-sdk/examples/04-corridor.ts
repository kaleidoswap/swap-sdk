/**
 * Arkade Intents corridor: bitcoin on Arkade <-> Lightning. ARKD is in the
 * maker's catalogue but is not a Boltz-shaped route — it is quoted over RFQ,
 * so `createReverseSwap({ to: "ARKD" })` is refused before any I/O.
 *
 * Funds nothing. Without ARK_PAYOUT_ADDRESS / ARK_PAYOUT_PUBKEY the maker
 * cannot decode the payout address and refuses, which is the path worth seeing.
 *
 *   node examples/04-corridor.ts
 */
import { randomBytes } from "node:crypto";

import {
  SwapClient,
  IntentsCorridor,
  init,
  isRfqQuote,
  newRfqId,
} from "@kaleidorg/swap-sdk";

import { die, network, optionalInt } from "./common.ts";

try {
  await init();

  const net = network();
  const client = SwapClient.forNetwork(net);
  const corridor = new IntentsCorridor(client);
  console.log(`corridor: ${client.corridorUrl}\n`);

  const amount = BigInt(optionalInt("ARK_RECEIVE_SATS", 12_000));

  // Your own preimage: its hash arms the hold invoice, the preimage claims.
  const paymentHash = randomBytes(32).toString("hex");

  const answer = await corridor.quoteLightningReceive({
    rfq_id: newRfqId(),
    // "to" = receive exactly this; the maker inverts it and can round up by a
    // sat or two, so assert >=, never equality.
    amount_side: "to",
    amount,
    payment_hash: paymentHash,
    payout_address:
      process.env.ARK_PAYOUT_ADDRESS ?? "tark1-not-a-real-address",
    payout_pubkey: process.env.ARK_PAYOUT_PUBKEY ?? "00".repeat(32),
  });

  if (!isRfqQuote(answer)) {
    console.log(`refused: ${answer.reason}  (rfq ${answer.rfq_id})`);
    console.log(
      "\nThat is the expected answer without a real Ark payout address — the " +
        "maker declined before reserving liquidity. Set ARK_PAYOUT_ADDRESS and " +
        "ARK_PAYOUT_PUBKEY for a live quote.",
    );
    process.exit(0);
  }

  console.log(`quote ${answer.rfq_id}`);
  console.log(`  pair:        ${answer.pair}`);
  console.log(`  you pay:     ${answer.from_amount} sat over Lightning`);
  console.log(`  you receive: ${answer.to_amount} sat on Arkade`);
  console.log(`  spread:      ${answer.from_amount - answer.to_amount} sat`);
  console.log(
    `  valid until: ${new Date(Number(answer.valid_until) * 1000).toISOString()}`,
  );

  // Funding the invoice *is* the acceptance: first check it pays payment_hash
  // for exactly from_amount, and that your covenant matches profile.lockup_address.
  console.log(
    `\n  invoice:     ${answer.profile.invoice ?? "(none returned)"}`,
  );

  const status = await client.rfqStatus(answer.rfq_id);
  console.log(`  state:       ${status.state}`);
} catch (error) {
  die(error);
}
