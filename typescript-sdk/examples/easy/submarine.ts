/**
 * Submarine swap with the high-level surface: on-chain BTC -> a Lightning
 * invoice. The same flow as ../02-submarine.ts, refund included: a swap that
 * unwinds after funding is refunded by the driver, or by `resume()` next run.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" BOLT11_INVOICE="lntbs…" REFUND_ADDRESS="tb1…" \
 *     node examples/easy/submarine.ts
 */
import {
  createKaleidoSwap,
  FileStore,
  isQuote,
  mnemonicSigner,
} from "@kaleidorg/swap-sdk";

const kaleido = await createKaleidoSwap({
  network: "signet",
  store: new FileStore(new URL("../swaps/", import.meta.url)),
  signer: mnemonicSigner(process.env.KALEIDO_SWAP_MNEMONIC!, "signet"),
});

await kaleido.resume();

const answer = await kaleido.quote({
  from: "bitcoin:BTC",
  to: "lightning:BTC",
  invoice: process.env.BOLT11_INVOICE!,
});
if (!isQuote(answer)) throw new Error(`${answer.reason}: ${answer.message}`);

const swap = await kaleido.start(answer, {
  refundAddress: process.env.REFUND_ADDRESS!,
});

const { fundingAddress, fundingAmount } = swap.action;
console.log(`Fund exactly ${fundingAmount} sat to:\n  ${fundingAddress}\n`);
for await (const phase of swap.phases()) console.log(`  ${phase}`);

console.log(`Done: the invoice was paid (${swap.id})`);
