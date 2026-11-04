/**
 * Reverse swap with the high-level surface: a Lightning payment -> L-BTC.
 * The same flow as ../advanced/03-reverse.ts, which is what this layer replaces.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" LIQUID_CLAIM_ADDRESS="tlq1…" \
 *     node examples/simple/reverse.ts
 */
import {
  createKaleidoSwap,
  FileStore,
  isQuote,
  isSwapError,
  mnemonicSigner,
} from "@kaleidorg/swap-sdk";

const kaleido = await createKaleidoSwap({
  network: "signet",
  store: new FileStore(new URL("../swaps/simple/", import.meta.url)),
  signer: mnemonicSigner(process.env.KALEIDO_SWAP_MNEMONIC!, "signet"),
});

try {
  const report = await kaleido.resume();
  if (report.errors.length)
    throw new Error(`resume could not move ${report.errors.map((e) => e.id)}`);

  const answer = await kaleido.quote({
    pay: "lightning:BTC",
    receive: "liquid:BTC",
    amount: { pay: 50_000 },
  });
  if (!isQuote(answer)) throw new Error(`${answer.reason}: ${answer.message}`);

  const swap = await kaleido.start(answer, {
    destination: process.env.LIQUID_CLAIM_ADDRESS!,
  });

  console.log(
    `Pay this invoice (${answer.fromAmount} sat):\n\n${swap.action.invoice}\n`,
  );
  for await (const phase of swap.phases()) console.log(`  ${phase}`);

  if (swap.phase !== "settled") {
    throw new Error(`swap ${swap.id} ended ${swap.phase}, not settled`);
  }
  console.log(
    `Done: ~${answer.toAmount} sat of L-BTC reached the claim address (${swap.id})`,
  );
} catch (error) {
  console.error(isSwapError(error) ? `${error.code}: ${error.message}` : error);
  process.exitCode = 1;
}
