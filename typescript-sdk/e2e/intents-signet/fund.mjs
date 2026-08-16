// Stage 2: fund a corridor lockup and watch it settle. THIS SPENDS.
//
// Run stage 1 (`probe.mjs`) first — a quote whose address the client refuses
// can never settle, so funding before that check only buys a stuck swap.
//
//   ARKADE_SEED=... INVOICE=lntbs... node fund.mjs --confirm
//
// The recovery record is written BEFORE the send, not after. `@arkade-os/swap`
// warns when a wallet cannot allocate an HD descriptor: the sender key is then
// random, and a lockup funded before that key is on disk is unrefundable if
// the solver never fills. Writing first costs nothing; writing after is a race
// against the only failure the refund path exists for.
import {
  InMemoryContractRepository, InMemoryWalletRepository,
  MnemonicIdentity, RestArkProvider, Wallet,
} from "@arkade-os/sdk";
import { requestLightningSend, httpTransport, rfqSecretsToRecord } from "@arkade-os/swap";
import bolt11 from "light-bolt11-decoder";
import { readFileSync, writeFileSync } from "node:fs";

const MAKER = process.env.MAKER_URL ?? "https://maker.signet.kaleidoswap.com";
const ARK = process.env.ARK_SERVER_URL ?? "https://mutinynet.arkade.sh";
const RECORD = process.env.RECORD_PATH ?? "./corridor-swap.json";

const seed = process.env.ARKADE_SEED?.trim();
const raw = process.env.INVOICE?.trim();
if (!seed || !raw) throw new Error("set ARKADE_SEED and INVOICE");
if (process.argv[2] !== "--confirm") throw new Error("refusing to spend without --confirm");

const wallet = await Wallet.create({
  identity: MnemonicIdentity.fromMnemonic(seed, {
    isMainnet: (process.env.ARK_NETWORK ?? "mutinynet") === "bitcoin",
  }),
  arkProvider: new RestArkProvider(ARK),
  settlementConfig: false, walletMode: "static",
  storage: {
    walletRepository: new InMemoryWalletRepository(),
    contractRepository: new InMemoryContractRepository(),
  },
});

const s = bolt11.decode(raw).sections;
const f = (n) => s.find((x) => x.name === n)?.value;
const invoice = {
  raw, paymentHash: f("payment_hash"),
  amountSats: Number(f("amount")) / 1000,
  expiresAt: Number(f("timestamp")) + Number(f("expiry") ?? 3600),
};

console.log(`balance  ${JSON.stringify((await wallet.getBalance()).available)} sats available`);
const payment = await requestLightningSend(wallet, ARK, httpTransport(MAKER), { invoice });
console.log(`quote    fund ${payment.fundAmount} for a ${invoice.amountSats} sat invoice`);
console.log(`lockup   ${payment.address}`);

// Everything needed to reconstruct or refund this swap, on disk first.
writeFileSync(RECORD, JSON.stringify({
  rfqId: payment.rfqId, address: payment.address,
  fundAmount: String(payment.fundAmount), invoice: raw,
  paymentHash: invoice.paymentHash,
  refundLocktime: payment.refundLocktime ?? null,
  secrets: payment.secrets ? rfqSecretsToRecord(payment.secrets) : null,
  fundedAt: null,
}, null, 1));
console.log(`record   ${RECORD} (written before funding)`);

const txid = await wallet.send({ address: payment.address, amount: payment.fundAmount });
console.log(`FUNDED   ${txid}`);
writeFileSync(RECORD, JSON.stringify({
  ...JSON.parse(readFileSync(RECORD, "utf8")),
  fundedAt: Math.floor(Date.now() / 1000), fundingTxid: txid,
}, null, 1));

console.log("\nthe solver should now pay the invoice and claim the lockup — watch the receiving node");
process.exit(0);
