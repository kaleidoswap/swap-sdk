/**
 * Reverse swap with the high-level surface: a Lightning payment -> L-BTC.
 * The same flow as ../03-reverse.ts, which is what this layer replaces.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" LIQUID_CLAIM_ADDRESS="tlq1…" \
 *     node examples/easy/reverse.ts
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

await kaleido.resume(); // claims or refunds anything a previous run left behind

const answer = await kaleido.quote({
  from: "lightning:BTC",
  to: "liquid:BTC",
  amount: 50_000,
});
if (!isQuote(answer)) throw new Error(`${answer.reason}: ${answer.message}`);

const swap = await kaleido.start(answer, {
  destination: process.env.LIQUID_CLAIM_ADDRESS!,
});

console.log(
  `Pay this invoice (${answer.fromAmount} sat):\n\n${swap.action.invoice}\n`,
);
for await (const phase of swap.phases()) console.log(`  ${phase}`);

console.log(
  `Done: ~${answer.toAmount} sat of L-BTC reached the claim address (${swap.id})`,
);
