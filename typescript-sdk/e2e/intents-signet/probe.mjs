// Live corridor probe against the deployed KaleidoSwap maker on signet.
//
// Stage 1: quote only. `requestLightningSend` derives the lockup and verifies
// it against the maker's quote BEFORE any funding, so this proves the whole
// protocol surface — wire shapes, quote fields, and byte-identical address
// derivation — without moving a satoshi. If the addresses disagreed, a real
// swap would die at `AddressMismatch` and never reach a spend anyway.
//
// The taker is `@arkade-os/swap`'s own client, reached through our venue's
// `flows` seam. Driving their client rather than a bespoke one is the point:
// a probe where our code talks to our code proves nothing about interop.
//
//   ARKADE_SEED="twelve words" INVOICE="lntbs..." node probe.mjs [http|nostr|both]
//
// The seed needs no funds for stage 1 — it only derives keys and a refund
// address. The invoice must be mutinynet and payable by the maker's node.
import {
  InMemoryContractRepository,
  InMemoryWalletRepository,
  MnemonicIdentity,
  RestArkProvider,
  Wallet,
} from "@arkade-os/sdk";
import { requestLightningSend, httpTransport } from "@arkade-os/swap";
import { nostrRfqTransport } from "@arkade-os/swap/nostr";

const MAKER = process.env.MAKER_URL ?? "https://maker.signet.kaleidoswap.com";
const ARK = process.env.ARK_SERVER_URL ?? "https://mutinynet.arkade.sh";
const RELAY = process.env.RELAY_URL ?? "wss://relay.kaleidoswap.com";
// Our solver's discovery key, as published in the kind-38859 card.
const SOLVER =
  process.env.SOLVER_PUBKEY ??
  "2183da8692e3ba0f96656b3bf84c59cd7e47b2e786a398a49390ce86e4d87346";

const seed = process.env.ARKADE_SEED?.trim();
const rawInvoice = process.env.INVOICE?.trim();
if (!seed) throw new Error("set ARKADE_SEED (no funds needed for stage 1)");
if (!rawInvoice) throw new Error("set INVOICE (mutinynet, payable by the maker)");

const which = (process.argv[2] ?? "both").toLowerCase();

const wallet = await Wallet.create({
  identity: MnemonicIdentity.fromMnemonic(seed),
  arkProvider: new RestArkProvider(ARK),
  settlementConfig: false,
  walletMode: "static",
  storage: {
    walletRepository: new InMemoryWalletRepository(),
    contractRepository: new InMemoryContractRepository(),
  },
});

const info = await new RestArkProvider(ARK).getInfo();
console.log(`ark      ${ARK}  network=${info.network} exitDelay=${info.unilateralExitDelay}`);
console.log(`maker    ${MAKER}`);

// Decoded here rather than pulled from a helper: the probe should depend on
// as little of our own code as possible, so a bug in ours cannot mask a bug
// in the exchange.
const bolt11 = (await import("light-bolt11-decoder")).default;
const sections = bolt11.decode(rawInvoice).sections;
const field = (name) => sections.find((s) => s.name === name)?.value;
const invoice = {
  raw: rawInvoice,
  paymentHash: field("payment_hash"),
  amountSats: Number(field("amount")) / 1000,
  expiresAt: Number(field("timestamp")) + Number(field("expiry") ?? 3600),
};
console.log(`invoice  ${invoice.amountSats} sats  hash=${invoice.paymentHash.slice(0, 16)}…`);

async function probe(label, transport) {
  console.log(`\n=== ${label} ===`);
  try {
    const payment = await requestLightningSend(wallet, ARK, transport, { invoice });
    // Reaching here means the client already re-derived the lockup and found
    // it equal to the maker's — `verifyLockupAddress` throws otherwise. That
    // equality is the whole interop claim, so it is worth naming.
    console.log(`✓ quote accepted, address verified by the client`);
    console.log(`  lockup   ${payment.address}`);
    console.log(`  fund     ${payment.fundAmount} sats`);
    console.log(`  spread   ${Number(payment.fundAmount) - invoice.amountSats} sats`);
    return true;
  } catch (error) {
    // AddressMismatch carries both sides; a refusal carries the solver's
    // reason. Either is far more useful than the message alone.
    console.log(`✗ ${error.constructor.name}: ${error.message}`);
    if (error.derived) console.log(`  client derived : ${error.derived}`);
    if (error.quoted) console.log(`  solver quoted  : ${error.quoted}`);
    return false;
  } finally {
    await transport.close?.();
  }
}

let ok = true;
if (which === "http" || which === "both") {
  ok = (await probe("HTTP transport", httpTransport(MAKER))) && ok;
}
if (which === "nostr" || which === "both") {
  // Nostr additionally proves the published card is usable: a wallet finds us
  // by the discovery key it advertises, with no base URL known in advance.
  ok = (await probe("Nostr transport", nostrRfqTransport({ relays: [RELAY], solverPubkey: SOLVER }))) && ok;
}

console.log(`\n${ok ? "PASS — nothing was funded" : "FAIL"}`);
process.exit(ok ? 0 : 1);
