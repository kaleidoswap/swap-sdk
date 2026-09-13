// Tests for the `@kaleidorg/swap-sdk/arkade` venue. Run against the built
// output (`npm test` builds first). Everything this venue does NOT own —
// `@arkade-os/swap`'s `RfqSwapManager` chain-evidence reads (`readLockupFate`,
// `findLockupVtxos`) — is exercised through a fake `indexerProvider` that
// answers `getVtxos`/`getVirtualTxs` the way a real Arkade indexer would;
// everything this venue DOES own (claim/refund dispatch, phase mapping,
// record persistence) is faked through the venue's own `flows` seam. Nothing
// here talks to a network or a real wallet — `wallet` can be an empty object
// because every path that would need it (`contractSigner`,
// `preimageForSwapRecord`) is reached only from the DEFAULT `flows.claimLockup`
// / `flows.refundArkade`, which every test below overrides.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash } from "node:crypto";

import { VHTLC, ArkAddress, Transaction } from "@arkade-os/sdk";
import { LockupNeedsRecoveryError } from "@arkade-os/swap";

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

function hexEncode(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
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
    nonInteractiveParameters: {
      receiverPkScript: p2tr(XONLY[1]),
      senderPkScript: p2tr(XONLY[0]),
      emulatorPubkey: XONLY[0],
    },
    ...overrides,
  };
}

function quote(overrides = {}) {
  const { profile, ...rest } = overrides;
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
    ...rest,
    profile: { payment_hash: "bb".repeat(32), ...profile },
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
    secrets: {
      descriptor: "tr(refund-descriptor)",
      pubkey: XONLY[0],
      pkScript: p2tr(XONLY[0]),
      address: "ark1qrefund",
    },
    treeParams: {},
    ...overrides,
  };
}

const RECEIVE_PREIMAGE = new Uint8Array(32).fill(6);

function receiveResponse(overrides = {}) {
  const script = new VHTLC.ScriptV2(vhtlcOptions());
  const payoutAddress = new ArkAddress(XONLY[2], XONLY[1]).encode();
  return {
    rfqId: overrides.rfqId ?? "rfq-r1",
    quote: quote({
      rfq_id: overrides.rfqId ?? "rfq-r1",
      pair: "lightning:BTC->arkade:BTC",
      profile: { payment_hash: sha256Hex(RECEIVE_PREIMAGE) },
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
    secrets: {
      descriptor: "tr(claim-descriptor)",
      pubkey: XONLY[1],
      preimage: RECEIVE_PREIMAGE,
      paymentHash: hexDecode(sha256Hex(RECEIVE_PREIMAGE)),
      mustPersistPreimage: true,
    },
    treeParams: {},
    ...overrides,
  };
}

/** A generic, always-open (unspent) vtxo — the default answer for the
 * `indexerProvider` any test that does not care about chain evidence gets.
 * Large enough to satisfy every `expectedAmountSats` used below. */
function openVtxo(overrides = {}) {
  return {
    txid: "cc".repeat(32),
    vout: 0,
    value: 10_000_000,
    isSpent: false,
    ...overrides,
  };
}

/** Build a fake indexer. `vtxos`/`recoverableVtxos` feed `getVtxos`;
 * `virtualTxs` (keyed by checkpoint txid) feed `getVirtualTxs` — both used
 * verbatim regardless of the query's `scripts` filter, which is fine since
 * every test below drives exactly one swap through the fake at a time. */
function fakeIndexer({
  vtxos = [openVtxo()],
  recoverableVtxos = [],
  virtualTxs = {},
} = {}) {
  return {
    getVtxos: async (params = {}) => ({
      vtxos: params.recoverableOnly ? recoverableVtxos : vtxos,
    }),
    getVirtualTxs: async (txids) => ({
      txs: txids.map((id) => virtualTxs[id]).filter(Boolean),
    }),
  };
}

/** A minimal, decodable PSBT spending `lockupTxid:lockupVout` with the given
 * final witness on its one input — enough for `readLockupFate` to read a
 * preimage (or not) out of `finalScriptWitness`. Returns both the encoded
 * PSBT and the transaction's own (real, computed) txid: `readLockupFate`'s
 * "returned" classification correlates a vtxo's `spentBy` against the
 * checkpoint's ACTUAL id, not an arbitrary label, so the fake vtxo must name
 * this same id. */
function spendTx(lockupTxid, lockupVout, witnessItems) {
  const tx = new Transaction({
    allowUnknown: true,
    allowUnknownOutputs: true,
    allowLegacyWitnessUtxo: true,
  });
  tx.addInput({
    txid: lockupTxid,
    index: lockupVout,
    witnessUtxo: { script: p2tr(XONLY[0]), amount: 1_000n },
    sequence: 0xfffffffd,
  });
  tx.addOutput({ script: p2tr(XONLY[1]), amount: 900n });
  tx.updateInput(0, { finalScriptWitness: witnessItems });
  return { psbt: Buffer.from(tx.toPSBT()).toString("base64"), txid: tx.id };
}

/** A fake indexer whose lockup was fully spent by a witness that does (or does
 * not) reveal `preimage` — drives the manager's own `readLockupFate` straight
 * to `claimed`/`returned` without going through this venue's claim/refund
 * seam at all (the "counterparty already resolved it" / "chain confirms our
 * own claim" path). */
function spentLockupIndexer({ lockupTxid, preimage, revealed }) {
  const witness = revealed ? [preimage] : [new Uint8Array(32).fill(0xee)];
  const { psbt, txid: checkpointTxid } = spendTx(lockupTxid, 0, witness);
  return fakeIndexer({
    vtxos: [
      {
        txid: lockupTxid,
        vout: 0,
        value: 1_050,
        spentBy: checkpointTxid,
        isSpent: true,
        arkTxId: "resolved-ark-tx",
      },
    ],
    virtualTxs: { [checkpointTxid]: psbt },
  });
}

function makeVenue({
  flows = {},
  now = () => NOW,
  store = new InMemoryArkadeSwapStore(),
  indexerProvider = fakeIndexer(),
} = {}) {
  const venue = new ArkadeIntentsVenue({
    wallet: {},
    arkServerUrl: "https://ark.example",
    transport: {},
    store,
    arkProvider: {},
    indexerProvider,
    now,
    flows: {
      requestLightningSend: async () => sendResponse(),
      requestLightningReceive: async () => receiveResponse(),
      claimLockup: async () => ({ arkTxid: "claim-tx", amount: 1_000 }),
      refundArkade: async () => null,
      ...flows,
    },
  });
  return { venue, store };
}

// ─── VHTLC option serialization ─────────────────────────────────────────────

test("vhtlc options survive the serialize/deserialize round trip (current shape)", () => {
  const options = vhtlcOptions();
  const serialized = serializeVhtlcOptions(options);
  assert.equal(serialized.nonInteractiveParameters.legacy, "current");
  const back = deserializeVhtlcOptions(serialized);
  const a = new VHTLC.ScriptV2(options);
  const b = new VHTLC.ScriptV2(back);
  assert.equal(a.claimScript, b.claimScript);
  assert.equal(a.refundScript, b.refundScript);
  // The current shape carries the third (timelocked, no-receiver-needed)
  // non-interactive leaf.
  assert.ok(a.nonInteractiveRefundWithoutReceiverScript);
  assert.ok(b.nonInteractiveRefundWithoutReceiverScript);
});

test("vhtlc options survive the round trip in the legacy (pre-timelocked-refund) shape", () => {
  const options = vhtlcOptions({
    nonInteractiveParameters: {
      ...vhtlcOptions().nonInteractiveParameters,
      legacy: "preTimelockedRefund",
    },
  });
  const serialized = serializeVhtlcOptions(options);
  assert.equal(
    serialized.nonInteractiveParameters.legacy,
    "preTimelockedRefund",
  );
  const back = deserializeVhtlcOptions(serialized);
  const a = new VHTLC.ScriptV2(options);
  const b = new VHTLC.ScriptV2(back);
  assert.equal(a.claimScript, b.claimScript);
  assert.equal(
    a.address("ark", XONLY[2]).encode(),
    b.address("ark", XONLY[2]).encode(),
  );
  // The legacy shape has no third leaf at all.
  assert.equal(a.nonInteractiveRefundWithoutReceiverScript, undefined);
  assert.equal(b.nonInteractiveRefundWithoutReceiverScript, undefined);
});

test("a serialized record with no legacy key at all decodes as legacy, never as current", () => {
  const options = vhtlcOptions();
  const serialized = serializeVhtlcOptions(options);
  // Simulate a record written before this field existed: strip the key
  // entirely rather than setting it to any particular value.
  delete serialized.nonInteractiveParameters.legacy;
  const back = deserializeVhtlcOptions(serialized);
  assert.equal(back.nonInteractiveParameters.legacy, "preTimelockedRefund");
  const rebuilt = new VHTLC.ScriptV2(back);
  assert.equal(rebuilt.nonInteractiveRefundWithoutReceiverScript, undefined);
});

test("a legacy-shaped record (old nonInteractiveClaim/nonInteractiveRefund fields) migrates to legacy", () => {
  const options = vhtlcOptions();
  const serialized = serializeVhtlcOptions(options);
  const oldShape = {
    senderHex: serialized.senderHex,
    receiverHex: serialized.receiverHex,
    serverHex: serialized.serverHex,
    preimageHashHex: serialized.preimageHashHex,
    refundLocktime: serialized.refundLocktime,
    unilateralClaimDelay: serialized.unilateralClaimDelay,
    unilateralRefundDelay: serialized.unilateralRefundDelay,
    unilateralRefundWithoutReceiverDelay:
      serialized.unilateralRefundWithoutReceiverDelay,
    nonInteractiveClaim: {
      receiverPkScriptHex:
        serialized.nonInteractiveParameters.receiverPkScriptHex,
      emulatorPubkeyHex: serialized.nonInteractiveParameters.emulatorPubkeyHex,
    },
    nonInteractiveRefund: {
      senderPkScriptHex: serialized.nonInteractiveParameters.senderPkScriptHex,
      emulatorPubkeyHex: serialized.nonInteractiveParameters.emulatorPubkeyHex,
    },
  };
  const back = deserializeVhtlcOptions(oldShape);
  assert.equal(back.nonInteractiveParameters.legacy, "preTimelockedRefund");
  assert.equal(
    hexEncode(back.nonInteractiveParameters.emulatorPubkey),
    hexEncode(XONLY[0]),
  );
});

// ─── prepare / notifyFunded ─────────────────────────────────────────────────

test("prepareLightningSend persists the record before returning", async () => {
  const { venue, store } = makeVenue();
  const prepared = await venue.prepareLightningSend({ invoice: {} });
  const stored = await store.get("rfq-1");
  assert.ok(stored, "record persisted");
  assert.equal(stored.phase, "prepared");
  assert.equal(stored.fundAmountSats, 1_050);
  assert.equal(stored.secrets.signingDescriptor, "tr(refund-descriptor)");
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

test("prepareLightningReceive persists secrets and payment hash material", async () => {
  const { venue, store } = makeVenue();
  const prepared = await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  const stored = await store.get("rfq-r1");
  assert.equal(stored.phase, "prepared");
  assert.equal(stored.expectedAmountSats, 1_000);
  assert.equal(stored.secrets.signingDescriptor, "tr(claim-descriptor)");
  assert.equal(stored.secrets.preimageHex, hexEncode(RECEIVE_PREIMAGE));
  assert.equal(prepared.invoice, "lnbc10n1example");
});

test("notifyFunded records the commitment and hands the swap to the manager", async () => {
  const { venue, store } = makeVenue();
  await venue.prepareLightningSend({ invoice: {} });
  const record = await venue.notifyFunded("rfq-1", "funding-txid");
  assert.equal(record.phase, "funded");
  assert.equal(record.fundingTxid, "funding-txid");
  assert.equal((await store.get("rfq-1")).phase, "funded");
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

// ─── prepared-record fast path (self-heal / cancel before the manager) ─────

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

test("a prepared send whose lockup is live self-heals to funded and is tracked", async () => {
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

test("reconcile cancels an unpaid receive after invoice expiry with no chain trace", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 301,
    flows: { readLockupFate: async () => ({ fate: "unknown" }) },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "cancelled");
});

test("a prepared receive that got secretly funded before notifyFunded self-heals", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 301, // past invoiceExpiresAt
    flows: { readLockupFate: async () => ({ fate: "open" }) },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "funded");
});

test("a transient readLockupFate failure on a prepared record retries next pass", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 601,
    flows: {
      readLockupFate: async () => {
        throw new Error("indexer down");
      },
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).phase, "prepared");
  assert.equal(report.errors.length, 0);
});

// ─── funded send: settle / refund / cancel via the manager ─────────────────

test("reconcile settles a funded send once chain evidence shows the claim", async () => {
  const preimage = new Uint8Array(32).fill(9);
  const paymentHash = sha256Hex(preimage);
  const lockupTxid = "11".repeat(32);
  const { venue, store } = makeVenue({
    indexerProvider: spentLockupIndexer({
      lockupTxid,
      preimage,
      revealed: true,
    }),
    flows: {
      requestLightningSend: async () =>
        sendResponse({ quote: { profile: { payment_hash: paymentHash } } }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", lockupTxid);
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["rfq-1"]);
  const record = await store.get("rfq-1");
  assert.equal(record.phase, "settled");
  assert.equal(record.resolvedTxid, "resolved-ark-tx");
});

test("reconcile refunds a matured send through the refundArkade flow", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      refundArkade: async () => ({ arkTxid: "refund-tx", amount: 1_050 }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.refunded, ["rfq-1"]);
  const record = await store.get("rfq-1");
  assert.equal(record.resolvedTxid, "refund-tx");
});

test("a matured send stays pending (unpushed) before its refund window opens", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 100, // funded, well before refund_locktime (NOW+3600)
    flows: {
      refundArkade: async () => {
        throw new Error("must not be called before refund_locktime");
      },
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).phase, "funded");
});

test("the solo refund of an empty (never actually funded) lockup reports cancelled, not refunded", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    indexerProvider: fakeIndexer({ vtxos: [] }), // chain never saw the lockup
    flows: { refundArkade: async () => null },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-1"]);
  assert.equal((await store.get("rfq-1")).resolvedTxid, undefined);
});

test("a swept lockup reports needs_recovery with its outpoints and keeps retrying", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601, // within the MTP-lag grace window
    flows: {
      refundArkade: async () => {
        throw new LockupNeedsRecoveryError(["deadbeef:0"], BigInt(NOW + 3600));
      },
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

test("a refund that keeps failing past the MTP-lag deadline ends failed", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3600 + 2 * 60 * 60 + 1, // past refund_locktime + REFUND_MTP_LAG_SECONDS
    flows: {
      refundArkade: async () => {
        throw new Error("server rejects: locktime not yet mature");
      },
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const report = await venue.reconcile();
  assert.deepEqual(report.failed, ["rfq-1"]);
  const record = await store.get("rfq-1");
  assert.equal(record.phase, "failed");
  assert.ok(record.failureReason);
});

test("refundSend triggers an immediate pass without waiting for the next reconcile", async () => {
  const { venue, store } = makeVenue({
    now: () => NOW + 3601,
    flows: {
      refundArkade: async () => ({ arkTxid: "direct-refund", amount: 1_050 }),
    },
  });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const record = await venue.refundSend("rfq-1");
  assert.equal(record.phase, "refunded");
  assert.equal((await store.get("rfq-1")).resolvedTxid, "direct-refund");
});

test("refundSend rejects for a receive-route record", async () => {
  const { venue } = makeVenue();
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await assert.rejects(() => venue.refundSend("rfq-r1"), /not a send swap/);
});

// ─── funded receive: claim / settle / refund / needs_recovery ─────────────

test("reconcile dispatches a claim once the lockup is funded", async () => {
  let claimInput;
  const { venue, store } = makeVenue({
    flows: {
      claimLockup: async (record, script, vtxos, options) => {
        claimInput = { record, vtxos, options };
        return { arkTxid: "claim-tx", amount: 1_000 };
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  // Our own submission is recorded, but the swap is not `settled` until
  // chain evidence confirms it — see the manager's own state-vs-fate split.
  assert.deepEqual(report.pending, ["rfq-r1"]);
  const record = await store.get("rfq-r1");
  assert.equal(record.managerClaimArkTxid, "claim-tx");
  assert.equal(claimInput.record.id, "rfq-r1");
  assert.equal(claimInput.options.partiallyClaimed, false);
  assert.ok(claimInput.vtxos.length > 0);
});

test("reconcile settles a receive once chain evidence confirms our claim", async () => {
  const lockupTxid = "33".repeat(32);
  const { venue, store } = makeVenue({
    indexerProvider: spentLockupIndexer({
      lockupTxid,
      preimage: RECEIVE_PREIMAGE,
      revealed: true,
    }),
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.settled, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "settled");
});

test("a claim past the wait window leaves the receive pending, not failed", async () => {
  const { venue, store } = makeVenue({
    flows: {
      claimLockup: async () => {
        throw new Error("relay hiccup");
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "funded");
});

test("a receive whose claim keeps failing past the deadline ends failed", async () => {
  // The manager only remembers a claim failure it actually attempted, so the
  // deadline has to be crossed AFTER at least one attempt inside the window
  // — not simply reconciled once, already past it.
  let now = NOW;
  const { venue, store } = makeVenue({
    now: () => now,
    flows: {
      claimLockup: async () => {
        throw new Error("wallet cannot sign");
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  let report = await venue.reconcile();
  assert.deepEqual(report.pending, ["rfq-r1"]);
  now = NOW + 3600 + 2 * 60 * 60 + 1;
  report = await venue.reconcile();
  assert.deepEqual(report.failed, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).phase, "failed");
});

test("a receive lockup swept before it could be claimed reports needs_recovery", async () => {
  const { venue, store } = makeVenue({
    flows: {
      claimLockup: async () => {
        throw new LockupNeedsRecoveryError(["swept:0"], BigInt(NOW + 3600));
      },
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.needsRecovery, ["rfq-r1"]);
  assert.deepEqual((await store.get("rfq-r1")).recoveryOutpoints, ["swept:0"]);
});

test("a receive past the claim window with no chain trace at all is cancelled", async () => {
  const { venue } = makeVenue({
    now: () => NOW + 3600 + 2 * 60 * 60 + 1,
    indexerProvider: fakeIndexer({ vtxos: [] }), // never funded
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.cancelled, ["rfq-r1"]);
});

test("a receive past the claim window that the solver reclaimed is refunded — a loss, not a cancel", async () => {
  const lockupTxid = "44".repeat(32);
  const { venue, store } = makeVenue({
    now: () => NOW + 3600 + 2 * 60 * 60 + 1,
    indexerProvider: spentLockupIndexer({
      lockupTxid,
      preimage: RECEIVE_PREIMAGE,
      revealed: false, // the solver's own reclaim, not our claim
    }),
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const report = await venue.reconcile();
  assert.deepEqual(report.refunded, ["rfq-r1"]);
  assert.equal((await store.get("rfq-r1")).resolvedTxid, "resolved-ark-tx");
});

test("claimReceive triggers a manager pass directly and returns the current record", async () => {
  const { venue, store } = makeVenue({
    flows: {
      claimLockup: async () => ({ arkTxid: "direct-claim", amount: 1_000 }),
    },
  });
  await venue.prepareLightningReceive({
    amountSats: 1_000,
    decodeInvoice: () => ({}),
  });
  await venue.notifyFunded("rfq-r1");
  const record = await venue.claimReceive("rfq-r1", { waitSeconds: 0 });
  assert.equal(record.managerClaimArkTxid, "direct-claim");
  assert.equal((await store.get("rfq-r1")).managerClaimArkTxid, "direct-claim");
});

test("claimReceive rejects for a send-route record", async () => {
  const { venue } = makeVenue();
  await venue.prepareLightningSend({ invoice: {} });
  await assert.rejects(() => venue.claimReceive("rfq-1"), /not a receive swap/);
});

// ─── reconcile concurrency ──────────────────────────────────────────────────

test("concurrent reconcile calls share one pass", async () => {
  let fateCalls = 0;
  const slowIndexer = {
    getVtxos: async (params) => {
      if (!params?.recoverableOnly) {
        fateCalls += 1;
        await new Promise((resolve) => setTimeout(resolve, 20));
      }
      return { vtxos: params?.recoverableOnly ? [] : [openVtxo()] };
    },
    getVirtualTxs: async () => ({ txs: [] }),
  };
  const { venue } = makeVenue({ indexerProvider: slowIndexer });
  await venue.prepareLightningSend({ invoice: {} });
  await venue.notifyFunded("rfq-1", "tx");
  const [a, b] = await Promise.all([venue.reconcile(), venue.reconcile()]);
  assert.equal(a, b, "second caller joins the running pass");
  assert.equal(fateCalls, 1, "the lockup is read once per pass, not raced");
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
    indexerProvider: indexer ?? {
      getVtxos: async () => ({ vtxos: [] }),
      getVirtualTxs: async () => ({ txs: [] }),
    },
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
      getVirtualTxs: async () => ({ txs: [] }),
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
      getVirtualTxs: async () => ({ txs: [] }),
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
      getVirtualTxs: async () => ({ txs: [] }),
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
      getVirtualTxs: async () => ({ txs: [] }),
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
    indexer: {
      getVtxos: async () => ({ vtxos: [] }),
      getVirtualTxs: async () => ({ txs: [] }),
    },
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

// ---------------------------------------------------------------------------
// kaleidoswapHttpTransport — the venue reached from the SDK's own `makerUrl`.
// ---------------------------------------------------------------------------

import {
  kaleidoswapHttpTransport,
  corridorRootFromMakerUrl,
} from "../dist/arkade/index.js";

test("kaleidoswapHttpTransport posts to /v1/swap and reads /v1/rfq beside the /v2 base", async () => {
  const calls = [];
  const quote = {
    v: 1,
    type: "rfq_quote",
    rfq_id: "abc",
    pair: "arkade:BTC->lightning:BTC",
    from_amount: 12120,
    to_amount: 12000,
    solver_pubkey: "02" + "ab".repeat(32),
    valid_until: 1_900_000_000,
    refund_locktime: 1_900_009_000,
    profile: {
      payment_hash: "00".repeat(32),
      lockup_address: "tark1q",
      receiver_pk_script: "5120aa",
    },
  };
  const status = {
    v: 1,
    type: "rfq_status",
    rfq_id: "abc",
    state: "settled",
    updated_at: 1,
    profile: {},
  };
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), method: init?.method ?? "GET" });
    const body = String(url).endsWith("/v1/swap") ? quote : status;
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };

  const transport = kaleidoswapHttpTransport("https://maker.example/v2", {
    fetchImpl,
  });
  const answer = await transport.requestQuote({
    v: 1,
    type: "rfq_request",
    rfq_id: "abc",
    pair: "arkade:BTC->lightning:BTC",
    amount_side: "to",
    profile: {},
  });
  assert.equal(answer.rfq_id, "abc");
  const seen = await transport.status("abc");
  assert.equal(seen?.state, "settled");

  assert.deepEqual(calls, [
    { url: "https://maker.example/v1/swap", method: "POST" },
    { url: "https://maker.example/v1/rfq/abc", method: "GET" },
  ]);
  assert.equal(
    corridorRootFromMakerUrl("https://maker.example/v2"),
    "https://maker.example",
  );
  await transport.close();
});

test("kaleidoswapHttpTransport refuses a base that is not a /v2 maker URL", () => {
  assert.throws(
    () => kaleidoswapHttpTransport("https://maker.example"),
    /\/v2/,
  );
});
