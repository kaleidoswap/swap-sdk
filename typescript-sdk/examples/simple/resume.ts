/**
 * Pick up every swap the store still holds: claim what is claimable, refund
 * what is refundable, then follow the rest to the end. Run it after a crash,
 * or at startup before anything else.
 *
 *   KALEIDO_SWAP_MNEMONIC="…12 words…" node examples/simple/resume.ts
 */
import {
  createKaleidoSwap,
  FileStore,
  mnemonicSigner,
} from "@kaleidorg/swap-sdk";

const kaleido = await createKaleidoSwap({
  network: "signet",
  store: new FileStore(new URL("../swaps/simple/", import.meta.url)),
  signer: mnemonicSigner(process.env.KALEIDO_SWAP_MNEMONIC!, "signet"),
});

const report = await kaleido.resume();
for (const r of report.settled) console.log(`${r.id}: settled`);
for (const r of report.refunded) console.log(`${r.id}: refunded`);
for (const r of report.failed)
  console.error(`${r.id}: failed (${r.rawStatus})`);
for (const e of report.errors)
  console.error(`${e.id}: ${e.code}: ${e.message}`);

const followed = await Promise.allSettled(
  report.resumed.map((swap) => swap.settled()),
);
for (const result of followed) {
  if (result.status === "fulfilled") console.log(`${result.value.id}: settled`);
  else console.error(`${result.reason.swapId}: ${result.reason.message}`);
}

if (
  report.failed.length ||
  report.errors.length ||
  followed.some((result) => result.status === "rejected")
)
  process.exitCode = 1;
