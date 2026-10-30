/**
 * Pick up every swap the store still holds: claim what is claimable, refund
 * what is refundable, then follow the rest to the end. Run it after a crash,
 * or at startup before anything else.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" node examples/easy/resume.ts
 */
import {
  createKaleidoSwap,
  FileStore,
  mnemonicSigner,
} from "@kaleidorg/swap-sdk";

const kaleido = await createKaleidoSwap({
  network: "signet",
  store: new FileStore(new URL("../swaps/", import.meta.url)),
  signer: mnemonicSigner(process.env.KALEIDO_SWAP_MNEMONIC!, "signet"),
});

const report = await kaleido.resume();
for (const r of report.settled) console.log(`${r.id}: settled`);
for (const r of report.refunded) console.log(`${r.id}: refunded`);
for (const r of report.failed) console.log(`${r.id}: failed (${r.rawStatus})`);

await Promise.all(
  report.resumed.map(async (swap) => {
    console.log(`${swap.id}: ${swap.phase}, following`);
    for await (const phase of swap.phases())
      console.log(`${swap.id}: ${phase}`);
  }),
);
