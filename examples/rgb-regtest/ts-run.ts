/** Live TS/wasm swaps. The native child owns only rgb-lib wallets, LN and regtest infrastructure. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile, rename } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { init, SwapClient, SwapMasterKey, SwapScript, type RgbPsbtParams, type ColoredRgbPsbt } from '../../typescript-sdk/dist/index.node.js';

const directory = fileURLToPath(new URL('.', import.meta.url));
const maker = 'http://127.0.0.1:29420/v2';
const esplora = 'http://127.0.0.1:23002';
const child = spawn(`${directory}target/debug/rgb-sdk-regtest`, ['ts-bridge'], { stdio: ['pipe', 'pipe', 'pipe'] });
let sequence = 0;
const pending = new Map<number, { resolve: (value: any) => void; reject: (reason: Error) => void }>();
const stdout = createInterface({ input: child.stdout });
stdout.on('line', line => {
  if (!line.startsWith('{')) { console.log(line); return; }
  const reply = JSON.parse(line);
  const p = pending.get(reply.id);
  if (!p) return;
  pending.delete(reply.id);
  if (reply.error) p.reject(new Error(reply.error)); else p.resolve(reply.result);
});
// Capture diagnostics privately: wallet libraries may include private swap material.
const diagnostics: string[] = [];
child.stderr.on('data', data => diagnostics.push(data.toString()));
child.on('error', error => {
  for (const p of pending.values()) p.reject(error);
  pending.clear();
});
child.on('exit', code => {
  for (const p of pending.values()) p.reject(new Error(`wallet bridge exited ${code}; inspect private run state`));
  pending.clear();
});
const json = (value: unknown) => JSON.stringify(value, (_, v) => {
  if (typeof v !== 'bigint') return v;
  assert(v >= 0n && v <= BigInt(Number.MAX_SAFE_INTEGER), 'regtest IPC integer exceeds safe range');
  return Number(v);
});
function call(command: string, params: Record<string, unknown> = {}): Promise<any> {
  const id = ++sequence;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    child.stdin.write(`${json({ id, command, ...params })}\n`);
  });
}
const pause = (ms: number) => new Promise(r => setTimeout(r, ms));
async function waitStatus(id: string, target: string, mine = false) {
  const deadline = Date.now() + 180_000;
  let last = '';
  for (let i = 0; Date.now() < deadline; i++) {
    const state = await client.swap(id);
    if (state.status !== last) { console.log(`swap ${id}: ${state.status}`); last = state.status; }
    if (state.status === target) return state;
    assert(!['transaction.failed', 'swap.expired', 'transaction.refunded.confirmed'].includes(state.status), `unexpected status ${state.status}`);
    if (mine && i % 5 === 0) await call('mine', { blocks: 1 });
    await pause(1000);
  }
  throw new Error(`swap ${id} did not reach ${target}; last ${last}`);
}
async function paymentStatus(paymentId: string, expected: number) {
  for (let i = 0; i < 60; i++) {
    const p = await call('payment', { paymentId });
    if (p.status === expected) return p;
    await pause(500);
  }
  throw new Error('Lightning payment did not reach expected terminal state');
}
async function persist(tag: string, state: unknown) { await call('persist', { tag, state }); }
await init();
const client = new SwapClient(maker, 30n);
const report: Record<string, any> = {
  makerRevision: process.env.RGB_MAKER_REVISION,
  sdkRevision: process.env.RGB_SDK_REVISION,
  rgbLibRevision: '96f039d975cf2a83712c3c0a90e703621d445425',
  network: 'regtest', bindings: 'TypeScript facade + WebAssembly',
};
try {
  const info = await call('info');
  const identity = JSON.parse(await readFile(`${directory}run/identity.json`, 'utf8'));
  const keys = SwapMasterKey.fromWalletMnemonic(identity.takerMnemonic, 'regtest');
  report.assetId = info.assetId; report.issued = info.issued;
  async function submarine(index: bigint, invoice: string, tag: string) {
    const key = keys.deriveSwapKey(index);
    const pairs = await client.submarinePairs();
    const response = await client.createSubmarineSwap('regtest', {
      from: 'USDT-RGB', to: 'BTC', invoice, refundPublicKey: key.publicKey,
      pairHash: pairs['USDT-RGB'].BTC.hash,
    }, info.assetId);
    assert(response.rgb.htlcSat <= 1000n);
    await persist(tag, { key, response });
    const lock = await call('lock', { lock: response.rgb });
    await persist(`${tag}-lock`, lock);
    return { response, key, lock };
  }
  const invoice = await call('invoice');
  const happy = await submarine(0n, invoice.invoice, 'submarine');
  await waitStatus(happy.response.id, 'transaction.claimed', true);
  await call('mine', { blocks: 1 });
  await paymentStatus(invoice.paymentId, 1);
  const remaining = info.issued - info.inventory - Number(happy.response.expectedAmount);
  await call('balance', { expected: remaining });
  report.submarine = { id: happy.response.id, lockTxid: happy.lock.txid, amount: Number(happy.response.expectedAmount), status: 'transaction.claimed', lightningPaymentSucceeded: true };
  console.log('PASS: TS submarine creation; real Lightning paid and maker claimed RGB');

  const hold = keys.derivePreimage(1n);
  await persist('hold', hold);
  const held = await call('invoice', { hash: hold.sha256 });
  const refund = await submarine(1n, held.invoice, 'submarine-refund');
  await waitStatus(refund.response.id, 'invoice.pending');
  await pause(2000);
  await call('failHold', { hash: hold.sha256 });
  await waitStatus(refund.response.id, 'invoice.failedToPay');
  const script = SwapScript.fromSubmarine('bitcoin', 'regtest', refund.response, refund.key.publicKey);
  const params: RgbPsbtParams = {
    outputAddress: await call('destination'), funding: { kind: 'callerInputs' }, maxFee: 5000n,
    swapId: refund.response.id, makerBaseUrl: maker, network: 'regtest', bitcoinEsploraUrl: esplora,
    lockupTxHex: refund.lock.hex,
  };
  await assert.rejects(script.prepareRgbRefund({ ...params, funding: { kind: 'htlcValue', feeRateSatVb: 5n } }), /rgb_fee_input_required|fee input|fee-input/i);
  let spend = await script.prepareRgbRefund(params);
  const funded = spend.fund(await call('fund', { template: spend.template() }));
  spend.free(); spend = funded;
  const refundColor = await call('color', { template: spend.template() });
  const colored: ColoredRgbPsbt = { ...refundColor.colored, allocations: refundColor.colored.allocations.map((a: any) => ({ ...a, amount: BigInt(a.amount) })) };
  const finalized = spend.finalizeRefund(colored, refund.key.secretKey);
  assert.equal(finalized.transaction, null, 'wallet fee input still needs signing');
  const signed = await call('signWallet', { psbt: finalized.psbt, swapInputIndex: finalized.swapInputIndex });
  const early = await call('mempool', { hex: signed.hex });
  assert.equal(early[0].allowed, false); assert.equal(early[0]['reject-reason'], 'non-final');
  await persist('submarine-refund-signed', { ...signed, operationId: refundColor.operationId });
  const timeout = Number(refund.response.timeoutBlockHeight);
  const height = await call('tip'); assert(height < timeout);
  await call('mine', { blocks: timeout - height + 1 });
  await call('broadcast', { ...signed, operationId: refundColor.operationId });
  await call('balance', { expected: remaining });
  const refundInspect = await call('inspect', { hex: signed.hex });
  assert.equal(refundInspect.witnessLength, 3); assert(refundInspect.hasRgbCommitment);
  report.submarineRefund = { id: refund.response.id, lockTxid: refund.lock.txid, amount: Number(refund.response.expectedAmount), ...refundInspect, earlyRejectReason: 'non-final', rgbBalanceRestored: true };
  spend.free(); script.free();
  console.log('PASS: TS submarine refund; early CLTV rejected, wallet fee signed, RGB restored');

  assert.equal((await call('balance', { wallet: 'reverse' })).btc, 0);
  async function reverse(index: bigint, tag: string) {
    const key = keys.deriveSwapKey(index); const preimage = keys.derivePreimage(index);
    const pairs = await client.reversePairs();
    const response = await client.createReverseSwap('regtest', {
      from: 'BTC', to: 'USDT-RGB', invoiceAmount: 100000n, claimPublicKey: key.publicKey,
      preimageHash: preimage.sha256, pairHash: pairs.BTC['USDT-RGB'].hash,
    }, info.assetId);
    assert(response.rgb.claimFeeRate >= 5n, 'quoted fee below local pre-payment policy');
    await persist(tag, { key, preimage, response });
    const paid = await call('pay', { invoice: response.invoice });
    await persist(`${tag}-payment`, paid);
    await waitStatus(response.id, 'transaction.mempool');
    const lock = await client.reverseTx(response.id);
    assert.equal(typeof lock.hex, 'string'); assert.equal(typeof lock.id, 'string');
    await persist(`${tag}-lock`, lock);
    await call('mine', { blocks: 1 });
    return { key, preimage, response, paid, lock };
  }
  const rev = await reverse(2n, 'reverse');
  await call('accept', { wallet: 'reverse', lock: rev.response.rgb, txid: rev.lock.id });
  const claimScript = SwapScript.fromReverse('bitcoin', 'regtest', rev.response, rev.key.publicKey);
  const claimSpend = await claimScript.prepareRgbClaim({
    outputAddress: await call('destination', { wallet: 'reverse' }),
    funding: { kind: 'htlcValue', feeRateSatVb: rev.response.rgb.claimFeeRate }, maxFee: 5000n,
    swapId: rev.response.id, makerBaseUrl: maker, network: 'regtest', bitcoinEsploraUrl: esplora, lockupTxHex: rev.lock.hex,
  });
  const claimColor = await call('color', { wallet: 'reverse', template: claimSpend.template() });
  const claimFinal = claimSpend.finalizeClaim({ ...claimColor.colored, allocations: claimColor.colored.allocations.map((a: any) => ({ ...a, amount: BigInt(a.amount) })) }, rev.key.secretKey, rev.preimage.preimage);
  assert(claimFinal.transaction, 'HTLC-funded reverse claim must be broadcastable');
  const claimHex = claimFinal.transaction.hex();
  const claimInspect = await call('inspect', { hex: claimHex, preimage: rev.preimage.preimage });
  assert.equal(claimInspect.inputCount, 1); assert.equal(claimInspect.witnessLength, 4);
  assert.equal(claimInspect.preimageMatches, true); assert(claimInspect.hasRgbCommitment);
  await persist('reverse-signed', { psbt: claimFinal.psbt, hex: claimHex, operationId: claimColor.operationId });
  await call('broadcast', { wallet: 'reverse', operationId: claimColor.operationId, psbt: claimFinal.psbt, hex: claimHex });
  const reverseAmount = Number(rev.response.rgb.amount);
  await call('balance', { wallet: 'reverse', expected: reverseAmount });
  await waitStatus(rev.response.id, 'invoice.settled'); await paymentStatus(rev.paid.paymentId, 1);
  report.reverse = { id: rev.response.id, lockTxid: rev.lock.id, amount: reverseAmount, htlcSat: Number(rev.response.rgb.htlcSat), walletBtcBefore: 0, ...claimInspect, status: 'invoice.settled', lightningPaymentSucceeded: true };
  claimFinal.transaction.free(); claimSpend.free(); claimScript.free();
  console.log('PASS: TS reverse claim; zero initial wallet BTC, real RGB received and hold invoice settled');

  const abandoned = await reverse(3n, 'reverse-refund');
  const abandonedTimeout = Number(abandoned.response.timeoutBlockHeight);
  assert((await client.swap(abandoned.response.id)).status.startsWith('transaction.'));
  const h = await call('tip'); assert(h < abandonedTimeout);
  await call('mine', { blocks: abandonedTimeout - h + 1 });
  // The RGB refund worker owns the maker's refund key; the taker never reveals its preimage.
  await waitStatus(abandoned.response.id, 'transaction.refunded.confirmed', true);
  await paymentStatus(abandoned.paid.paymentId, 2);
  const outputs = await (await fetch(`${esplora}/tx/${abandoned.lock.id}/outspends`)).json();
  let refundHex: string | undefined;
  for (const output of outputs) {
    if (!output.spent) continue;
    const hex = await (await fetch(`${esplora}/tx/${output.txid}/hex`)).text();
    const inspection = await call('inspect', { hex });
    if (inspection.locktime === abandonedTimeout && inspection.witnessLength === 3) { refundHex = hex; break; }
  }
  assert(refundHex, 'confirmed maker refund transaction not found');
  const reverseRefundInspect = await call('inspect', { hex: refundHex });
  assert(reverseRefundInspect.hasRgbCommitment);
  report.reverseRefund = { id: abandoned.response.id, lockTxid: abandoned.lock.id, amount: Number(abandoned.response.rgb.amount), ...reverseRefundInspect, status: 'transaction.refunded.confirmed', lightningPaymentFailed: true, makerRgbBalanceRestored: true };
  console.log('PASS: abandoned TS reverse swap; maker timeout refund confirmed and Lightning released');

  const makerExpected = info.inventory + Number(happy.response.expectedAmount) - reverseAmount;
  const audited = await call('audit', { expected: makerExpected });
  assert.equal(audited.maker + remaining + reverseAmount, info.issued);
  report.settledBalances = { maker: audited.maker, taker: remaining, reverse: reverseAmount, total: info.issued };
  report.completedAt = new Date().toISOString();
  await writeFile(`${directory}run/ts-report.tmp`, json(report), { mode: 0o600 });
  await rename(`${directory}run/ts-report.tmp`, `${directory}run/ts-report.json`);
  console.log('PASS: all four TS flows; settled RGB balances conserve the full issuance');
} catch (error) {
  await writeFile(`${directory}run/ts-failure.json`, json({ error: String(error), diagnostics }), { mode: 0o600 });
  console.error('Live TS validation stopped; detailed diagnostics are in private run/ts-failure.json');
  process.exitCode = 1;
} finally {
  client.free(); child.stdin.end();
}
