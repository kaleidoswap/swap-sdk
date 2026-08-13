// Tests for the `@kaleidorg/swap-sdk/arkade` venue. Run against the built
// output (`npm test` builds first), with every `@arkade-os/swap` flow faked
// through the venue's `flows` seam — nothing here talks to a network or a
// real wallet. The fallback ("stored") secrets arm keeps identity/preimage
// derivation off the wallet entirely, so `wallet` can be an empty object.
import assert from "node:assert/strict";
import { test } from "node:test";

import { VHTLC, ArkAddress } from "@arkade-os/sdk";
import { randomSwapSecrets } from "@arkade-os/swap";

import {
  ArkadeIntentsVenue,
  InMemoryArkadeSwapStore,
  deserializeVhtlcOptions,
  serializeVhtlcOptions,
} from "../dist/arkade/index.js";

// Valid x-only points (secp256k1 G.x, 2G.x, BIP-340 vector) so real script
// and address constructions never trip point validation.
const XONLY = [
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
].map(hexDecode);

function hexDecode(value) {
  const out = new Uint8Array(value.length / 2);
  for (let i = 0; i < out.length; i++)
    out[i] = Number.parseInt(value.slice(i * 2, i * 2 + 2), 16);
  return out;
}

const NOW = 1_800_000_000;

// A valid P2TR pkScript: OP_1 PUSH32 <x-only key>.
function p2tr(key) {
  return new Uint8Array([0x51, 0x20, ...key]);
}

function vhtlcOptions(overrides = {}) {
  return {
    sender: XONLY[0],
    receiver: XONLY[1],
    server: XONLY[2],
    preimageHash: new Uint8Array(20).fill(7),
    refundLocktime: BigInt(NOW + 3600),
    unilateralClaimDelay: { type: "seconds", value: 512n },
    unilateralRefundDelay: { type: "seconds", value: 1024n },
    unilateralRefundWithoutReceiverDelay: { type: "seconds", value: 1536n },
    nonInteractiveClaim: {
      receiverPkScript: p2tr(XONLY[1]),
      emulatorPubkey: XONLY[0],
    },
    nonInteractiveRefund: {
      senderPkScript: p2tr(XONLY[0]),
      emulatorPubkey: XONLY[0],
    },
    ...overrides,
  };
}

function quote(overrides = {}) {
  return {
    v: 1,
    type: "rfq_quote",
    rfq_id: overrides.rfq_id ?? "rfq-1",
    pair: "arkade:BTC->lightning:BTC",
    from_amount: 1_050,
    to_amount: 1_000,
    solver_pubkey: "aa".repeat(32),
    valid_until: NOW + 600,
    refund_locktime: NOW + 3600,
    profile: { payment_hash: "bb".repeat(32) },
    ...overrides,
  };
}

function sendResponse(overrides = {}) {
  const script = new VHTLC.ScriptV2(vhtlcOptions());
  return {
    rfqId: overrides.rfqId ?? "rfq-1",
    quote: quote({ rfq_id: overrides.rfqId ?? "rfq-1", ...overrides.quote }),
    address: "ark1qexample",
    fundAmount: 1_050,
    swapPkScript: new Uint8Array(34).fill(3),
    script,
    refundAddress: "ark1qrefund",
    senderPubkey: XONLY[0],
    secrets: randomSwapSecrets(),
    ...overrides,
  };
}

function receiveResponse(overrides = {}) {
  const script = new VHTLC.ScriptV2(vhtlcOptions());
  const payoutAddress = new ArkAddress(XONLY[2], XONLY[1]).encode();
  return {
    rfqId: overrides.rfqId ?? "rfq-r1",
    quote: quote({
      rfq_id: overrides.rfqId ?? "rfq-r1",
      pair: "lightning:BTC->arkade:BTC",
      ...overrides.quote,
    }),
    invoice: "lnbc10n1example",
    payAmount: 1_050,
    expectedAmount: 1_000,
    invoiceExpiresAt: NOW + 300,
    address: "ark1qlockup",
    swapPkScript: new Uint8Array(34).fill(4),
    script,
    payoutAddress,
    payoutPubkey: XONLY[1],
    secrets: randomSwapSecrets({ preimage: true }),
    ...overrides,
  };
}

function makeVenue({
  flows = {},
  now = () => NOW,
  store = new InMemoryArkadeSwapStore(),
} = {}) {
  const venue = new ArkadeIntentsVenue({
    wallet: {},
    arkServerUrl: "https://ark.example",
    transport: {},
    store,
    arkProvider: {},
    indexerProvider: {},
    now,
    flows: {
      requestLightningSend: async () => sendResponse(),
      requestLightningReceive: async () => receiveResponse(),
      claimReceiveLockup: async () => ({ arkTxid: "claim-tx", amount: 1_000 }),
      refundIfUnresolved: async () => ({
        outcome: "nothing_to_refund",
        status: null,
      }),
      readLockupFate: async () => ({ fate: "open" }),
      ...flows,
    },
  });
  return { venue, store };
}

test("vhtlc options survive the serialize/deserialize round trip", () => {
  const options = vhtlcOptions();
  const back = deserializeVhtlcOptions(serializeVhtlcOptions(options));
  assert.deepEqual(back, options);
  // And the rebuilt covenant is byte-identical where it matters.
  const a = new VHTLC.ScriptV2(options);
  const b = new VHTLC.ScriptV2(back);
  assert.equal(a.claimScript, b.claimScript);
  assert.equal(a.refundScript, b.refundScript);
});

test("prepareLightningSend persists the record before returning", async () => {
  const { venue, store } = makeVenue();
  const prepared = await venue.prepareLightningSend({ invoice: {} });
  const stored = await store.get("rfq-1");
  assert.ok(stored, "record persisted");
  assert.equal(stored.phase, "prepared");
  assert.equal(stored.fundAmountSats, 1_050);
  assert.equal(prepared.summary.feeSats, 50);
  assert.equal(prepared.summary.venue, "arkade-intents");
  assert.equal(prepared.address, "ark1qexample");
});

test("a store failure surfaces before any funding instruction exists", async () => {
  const failing = {
    put: async () => {
      throw new Error("disk full");
    },
    get: async () => undefined,
    listPending: async () => [],
  };
  const { venue } = makeVenue({ store: failing });
  await assert.rejects(
    () => venue.prepareLightningSend({ invoice: {} }),
    /disk full/,
  );
});

test("notifyFunded records the commitment", async () => {
  const { venue } = makeVenue();
  await venue.prepareLightningSend({ invoice: {} });
  const record = await venue.notifyFunded("rfq-1", "funding-txid");
  assert.equal(record.phase, "funded");
  assert.equal(record.fundingTxid, "funding-txid");
});

test("reconcile cancels an expired send only when the chain saw no lockup", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 601,
    flows: { readLockupFate: async () => ({ fate: "unknown" }) },
  });
  await venue.prepareLightningSend({ invoice: {} });
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).phase, "cancelled");
});

test("a prepared send whose lockup is live self-heals to funded", async () => {
  // Funding is acceptance: the host can broadcast and crash before
  // notifyFunded. The record must never leave the pending set while the
  // chain shows a live lockup.
  const { venue, store } = makeVenue({
    now: () => NOW + 601, // past valid_until, before refund_locktime
    flows: { readLockupFate: async () => ({ fate: "open" }) },
  });
  await venue.prepareLightningSend({ invoice: {} });
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).phase, "funded");
});

test("a crashed-before-notify send still refunds after the locktime", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      readLockupFate: async () => ({ fate: "open" }),
      refundIfUnresolved: async () => ({
        outcome: "refunded",
        arkTxid: "late-refund-tx",
        amount: 1_050,
        status: null,
      }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  // No notifyFunded at all — reconcile alone must recover the funds.
  const report = await venue.reconcile();
  assert.deepEqual(report.refunded, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).resolvedTxid, "late-refund-tx");
});

test("reconcile settles a funded send whose lockup was claimed", async () => {
  const { venue } = makeVenue({
    flows: {
      readLockupFate: async () => ({
        fate: "claimed",
        preimage: new Uint8Array(32),
      }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["rfq-1"]);
});

test("reconcile refunds a matured send through refundIfUnresolved", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      refundIfUnresolved: async () => ({
        outcome: "refunded",
        arkTxid: "refund-tx",
        amount: 1_050,
        status: null,
      }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.refunded, ["rfq-1"]);
  const record = await store.get("rfq-1");
  assert.equal(record.resolvedTxid, "refund-tx");
});

test("a swept lockup lands in needs_recovery with its outpoints", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      refundIfUnresolved: async () => ({
        outcome: "needs_recovery",
        outpoints: ["deadbeef:0"],
        vtxos: [],
        status: null,
      }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.needsRecovery, ["rfq-1"]);
  assert.deepEqual((await store.get("rfq-1")).recoveryOutpoints, [
    "deadbeef:0",
  ]);
});

test("reconcile claims a funded receive and settles it", async () => {
  let claimInput;
  const { venue, store } = makeVenue({
    flows: {
      claimReceiveLockup: async (_indexer, _ark, input) => {
        claimInput = input;
        return { arkTxid: "claim-tx", amount: 1_000 };
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).resolvedTxid, "claim-tx");
  assert.equal(claimInput.expectedAmount, 1_000);
  // The claim deadline never exceeds the solver's refund horizon.
  assert.ok(claimInput.deadline <= NOW + 3600);
});

test("a receive past the claim window settles when covclaimd claimed it", async () => {
  // covclaimdPubkey exists so the solver's claim daemon can claim while the
  // wallet is offline; that claim IS the settlement, even if this venue
  // only learns about it after refund_locktime.
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      readLockupFate: async () => ({
        fate: "claimed",
        preimage: new Uint8Array(32),
      }),
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "settled");
});

test("a receive past the claim window cancels only when never funded", async () => {
  const { venue } = makeVenue({
    now: () => NOW + 3601,
    flows: { readLockupFate: async () => ({ fate: "unknown" }) },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-r1"]);
});

test("a receive still open past the deadline keeps watching", async () => {
  // Claiming past refund_locktime races the solver's refund; the lockup is
  // the solver's to resolve, so the record stays pending until the chain
  // shows claimed or returned.
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: { readLockupFate: async () => ({ fate: "open" }) },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "funded");
});

test("reconcile cancels an unpaid receive after invoice expiry", async () => {
  const { venue } = makeVenue({ now: () => NOW + 301 });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-r1"]);
});

test("one record's failure never blocks the rest of the pass", async () => {
  const { venue } = makeVenue({
    flows: {
      readLockupFate: async () => {
        throw new Error("indexer down");
      },
      requestLightningSend: async () => sendResponse(),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.equal(report.errors.length, 1);
  assert.equal(report.errors[0].id, "rfq-1");
});

test("a claim past the wait window leaves the receive pending", async () => {
  const { venue, store } = makeVenue({
    flows: {
      claimReceiveLockup: async () => {
        throw new Error("funding_wait_deadline");
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    covclaimdPubkey: new Uint8Array(33).fill(2),
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "funded");
});

test("notifyFunded refuses to resurrect a terminal record", async () => {
  const { venue, store } = makeVenue();
  await venue.prepareLightningSend({ invoice: {} });
  const record = await store.get("rfq-1");
  record.phase = "settled";
  await store.put(record);
  await assert.rejects(() => venue.notifyFunded("rfq-1", "tx"), /settled/);
  assert.equal((await store.get("rfq-1")).phase, "settled");
  // While a repeat on an already-funded record is an idempotent retry.
  record.phase = "funded";
  await store.put(record);
  const updated = await venue.notifyFunded("rfq-1", "tx-2");
  assert.equal(updated.phase, "funded");
});

test("nothing_to_refund maps through chain evidence, not fundingTxid", async () => {
  const fateCase = async (fate, expectedPhase) => {
    const { venue, store } = makeVenue({
      now: () => NOW + 3601,
      flows: {
        refundIfUnresolved: async () => ({
          outcome: "nothing_to_refund",
          status: null,
        }),
        readLockupFate: async () =>
          fate === "claimed"
            ? { fate, preimage: new Uint8Array(32) }
            : { fate },
      },
    });
    await venue.prepareLightningSend({ invoice: {} });
    // notifyFunded WITHOUT a txid — the shape the old shortcut misread.
    await venue.notifyFunded("rfq-1");
    await venue.refundSend("rfq-1");
    assert.equal(
      (await store.get("rfq-1")).phase,
      expectedPhase,
      `fate ${fate}`,
    );
  };
  await fateCase("claimed", "settled");
  await fateCase("returned", "refunded");
  await fateCase("unknown", "cancelled");
});

test("concurrent reconcile calls share one pass", async () => {
  let fateCalls = 0;
  const { venue } = makeVenue({
    flows: {
      readLockupFate: async () => {
        fateCalls += 1;
        await new Promise((resolve) => setTimeout(resolve, 20));
        return { fate: "open" };
      },
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const [a, b] = await Promise.all([venue.reconcile(), venue.reconcile()]);
  assert.equal(a, b, "second caller joins the running pass");
  assert.equal(fateCalls, 1, "records are read once, not raced");
});

test("a quote without refund_locktime falls back to the covenant's", async () => {
  let refundInput;
  const { venue, store } = makeVenue({
    now: () => NOW + 3601, // past the script's refundLocktime (NOW + 3600)
    flows: {
      requestLightningSend: async () => {
        const response = sendResponse();
        delete response.quote.refund_locktime;
        return response;
      },
      readLockupFate: async () => ({ fate: "open" }),
      refundIfUnresolved: async (_t, _a, _i, input) => {
        refundInput = input;
        return {
          outcome: "refunded",
          arkTxid: "fallback-refund",
          amount: 1_050,
          status: null,
        };
      },
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.refunded, ["rfq-1"]);
  assert.equal(report.errors.length, 0, "no eternal error loop");
  assert.equal(refundInput.refundLocktime, NOW + 3600);
  assert.equal((await store.get("rfq-1")).resolvedTxid, "fallback-refund");
});

// ─── Asset-swap route ───────────────────────────────────────────────────────

import { InMemoryAssetSwapRepository, updateAssetSwap } from "@arkade-os/swap";

function assetVenue({ flows = {}, indexer, now = () => NOW } = {}) {
  const repository = new InMemoryAssetSwapRepository();
  const venue = new ArkadeIntentsVenue({
    wallet: {},
    arkServerUrl: "https://ark.example",
    transport: {},
    store: new InMemoryArkadeSwapStore(),
    assetSwapRepository: repository,
    arkProvider: {
      getInfo: async () => ({ signerPubkey: "02" + "aa".repeat(32) }),
    },
    indexerProvider: indexer ?? { getVtxos: async () => ({ vtxos: [] }) },
    now,
    flows: {
      createOffer: async () => ({
        offerHex: "0f0f",
        extension: { type: 3, payload: new Uint8Array([1]) },
        address: "ark1qoffer",
        swapPkScript: new Uint8Array(34).fill(9),
      }),
      cancelOffer: async () => "cancel-txid",
      classifyAssetSwapSpend: async () => "indeterminate",
      ...flows,
    },
  });
  return { venue, repository };
}

async function fundedAssetSwap(venue) {
  const prepared = await venue.prepareAssetSwap({
    wantAmountAtomic: 100n,
    wantAssetId: "f1".repeat(34),
  });
  return venue.notifyAssetSwapFunded({
    prepared,
    fundingTxid: "fund-tx",
    fromAssetId: "btc",
    toAssetId: "f1".repeat(34),
    fromAmountAtomic: 1_000n,
    toAmountAtomic: 100n,
  });
}

test("a funded asset swap is persisted pending, keyed by funding txid", async () => {
  const { venue, repository } = assetVenue();
  const swap = await fundedAssetSwap(venue);
  assert.equal(swap.id, "fund-tx");
  assert.equal(swap.status, "pending");
  const stored = await repository.getAllSwaps();
  assert.equal(stored.length, 1);
  assert.equal(stored[0].offerHex, "0f0f");
});

test("reconcile settles an asset swap whose deposit was filled", async () => {
  const { venue, repository } = assetVenue({
    indexer: {
      getVtxos: async () => ({
        vtxos: [
          { txid: "fund-tx", vout: 0, spentBy: "fill-tx", isSpent: true },
        ],
      }),
    },
    flows: { classifyAssetSwapSpend: async () => "fulfilled" },
  });
  await fundedAssetSwap(venue);
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["fund-tx"]);
  const [stored] = await repository.getAllSwaps();
  assert.equal(stored.status, "fulfilled");
  assert.equal(stored.spentTxid, "fill-tx");
});

test("an unspent asset-swap deposit stays pending forever — no expiry", async () => {
  const { venue, repository } = assetVenue({
    indexer: {
      getVtxos: async () => ({ vtxos: [{ txid: "fund-tx", vout: 0 }] }),
    },
    now: () => NOW + 10_000_000, // months later; offers never time out
  });
  await fundedAssetSwap(venue);
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["fund-tx"]);
  assert.equal((await repository.getAllSwaps())[0].status, "pending");
});

test("an unclassifiable spend is left alone, never guessed", async () => {
  const { venue, repository } = assetVenue({
    indexer: {
      getVtxos: async () => ({
        vtxos: [{ txid: "fund-tx", vout: 0, spentBy: "spend-tx" }],
      }),
    },
    flows: { classifyAssetSwapSpend: async () => "indeterminate" },
  });
  await fundedAssetSwap(venue);
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["fund-tx"]);
  assert.equal((await repository.getAllSwaps())[0].status, "pending");
});

test("cancelAssetSwap losing the race to a fill reports fulfilled", async () => {
  const { venue, repository } = assetVenue({
    indexer: {
      getVtxos: async () => ({
        vtxos: [{ txid: "fund-tx", vout: 0, spentBy: "fill-tx" }],
      }),
    },
    flows: {
      cancelOffer: async () => {
        throw new Error("deposit already spent");
      },
      classifyAssetSwapSpend: async () => "fulfilled",
    },
  });
  await fundedAssetSwap(venue);
  const outcome = await venue.cancelAssetSwap("fund-tx");
  assert.equal(outcome.status, "fulfilled");
  assert.equal((await repository.getAllSwaps())[0].status, "fulfilled");
});

test("cancelAssetSwap rethrows when the chain answers nothing", async () => {
  const { venue } = assetVenue({
    indexer: { getVtxos: async () => ({ vtxos: [] }) },
    flows: {
      cancelOffer: async () => {
        throw new Error("relay hiccup");
      },
    },
  });
  await fundedAssetSwap(venue);
  await assert.rejects(() => venue.cancelAssetSwap("fund-tx"), /relay hiccup/);
});

test("cancelAssetSwap happy path returns the repository's view", async () => {
  const { venue, repository } = assetVenue({
    flows: {
      cancelOffer: async (_w, _u, _hex, opts) => {
        await updateAssetSwap(opts.repository, "fund-tx", {
          status: "cancelled",
        });
        return "cancel-txid";
      },
    },
  });
  await fundedAssetSwap(venue);
  const outcome = await venue.cancelAssetSwap("fund-tx");
  assert.equal(outcome.status, "cancelled");
  assert.equal((await repository.getAllSwaps())[0].status, "cancelled");
});
