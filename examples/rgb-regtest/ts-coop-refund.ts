/** Live TS/wasm swaps. The native child owns only rgb-lib wallets, LN and regtest infrastructure. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile, rename } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { init, SwapClient, SwapMasterKey, SwapScript, type RgbPsbtParams, type ColoredRgbPsbt, getRgbRefundPartialSig } from '../../typescript-sdk/dist/index.node.js';

const directory = fileURLToPath(new URL('.', import.meta.url));
const maker = 'http://127.0.0.1:29420/v2';
const esplora = 'http://127.0.0.1:23002';
const child = spawn(`${directory}target/debug/rgb-sdk-regtest`, ['ts-attach'], { stdio: ['pipe', 'pipe', 'pipe'] });
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
const report: Record<string, unknown> = { protocol: 'rgb-coop-refund-v1', network: 'regtest' };
try {
  const info = await call('info');
  const identity = JSON.parse(await readFile(`${directory}run/identity.json`, 'utf8'));
  const keys = SwapMasterKey.fromWalletMnemonic(identity.takerMnemonic, 'regtest');
  const index = BigInt(process.env.RGB_COOP_TEST_INDEX ?? '10');
  const tag = `coop-refund-${index}`;
  const recover = process.env.RGB_COOP_RECOVER === '1';
  let before = await call('balance');
  let key: any; let response: any; let lock: any; let spend: any; let color: any;
  let pendingHoldHash: string | undefined;
  if (recover) {
    const saved = JSON.parse(await readFile(`${directory}run/ts-${tag}.json`, 'utf8'));
    key = saved.key; response = saved.response;
    response.rgb.amount = BigInt(response.rgb.amount);
    lock = JSON.parse(await readFile(`${directory}run/ts-${tag}-lock.json`, 'utf8'));
    const request = JSON.parse(await readFile(`${directory}run/ts-${tag}-request.json`, 'utf8'));
    color = await call('restoreOperation', { operationId: request.operationId });
    before = { ...before, rgb: before.rgb + Number(response.rgb.amount) };
  } else {
    await assert.rejects(readFile(`${directory}run/ts-${tag}.json`, 'utf8'), { code: 'ENOENT' });
    key = keys.deriveSwapKey(index); const hold = keys.derivePreimage(index);
    await persist(`${tag}-preimage`, hold);
    const invoice = await call('invoice', { hash: hold.sha256 });
    const pairs = await client.submarinePairs();
    response = await client.createSubmarineSwap('regtest', {
      from: 'USDT-RGB', to: 'BTC', invoice: invoice.invoice, refundPublicKey: key.publicKey,
      pairHash: pairs['USDT-RGB'].BTC.hash,
    }, info.assetId);
    assert.equal(response.rgb.cooperativeRefund, 'rgb-coop-refund-v1');
    await persist(tag, { key, response, before });
    lock = await call('lock', { lock: response.rgb });
    await persist(`${tag}-lock`, lock);
    await waitStatus(response.id, 'invoice.pending');
    pendingHoldHash = hold.sha256;
  }
  const script = SwapScript.fromSubmarine('bitcoin', 'regtest', response, key.publicKey);
  const params: RgbPsbtParams = {
    outputAddress: recover ? color.outputAddress : await call('destination'), funding: { kind: 'callerInputs' }, maxFee: 5000n,
    swapId: response.id, makerBaseUrl: maker, network: 'regtest', bitcoinEsploraUrl: esplora,
    lockupTxHex: lock.hex,
  };
  spend = await script.prepareRgbCooperativeRefund(params);
  const funded = spend.fund(recover ? color.fundedPsbt : await call('fund', { template: spend.template() }));
  spend.free(); spend = funded;
  const template = spend.template();
  if (!recover) color = await call('color', { template });
  const colored: ColoredRgbPsbt = { ...color.colored, allocations: color.colored.allocations.map((a: any) => ({ ...a, amount: BigInt(a.amount) })) };
  const [lockedTxid, lockedVout] = template.swapOutpoint.split(':');
  const validate = { psbt: colored.psbt, txid: lockedTxid, vout: Number(lockedVout), amount: response.rgb.amount };
  await call('validateRefund', validate);
  await assert.rejects(call('validateRefund', { ...validate, amount: response.rgb.amount - 1n }));
  for (const mutation of ['commitment', 'proof']) {
    const psbt = await call('mutatePsbt', { psbt: colored.psbt, mutation });
    await assert.rejects(call('validateRefund', { ...validate, psbt }));
  }
  const session = spend.beginCooperativeRefund(colored, key.secretKey, response.id);
  const request = session.request();
  await persist(`${tag}-request`, { request, operationId: color.operationId });
  async function rawRequest(body: unknown, auth?: string) {
    const r = await fetch(`${maker}/swap/submarine/${response.id}/refund`, {
      method: 'POST', headers: { 'content-type': 'application/json', ...(auth ? { 'X-Swap-Auth': auth } : {}) }, body: json(body),
    });
    return { status: r.status, body: await r.json() };
  }
  assert.equal((await rawRequest(request)).status, 401);
  assert.equal((await rawRequest(request, 'invalid')).status, 401);
  for (const [i, mutation] of ['commitment', 'proof', 'prevout'].entries()) {
    const psbt = await call('mutatePsbt', { psbt: request.psbt, index: request.index, mutation });
    const r = await rawRequest({ ...request, psbt, sessionId: String(i+1).repeat(64) }, response.swapAuth);
    assert.equal(r.status, 422, `maker accepted invalid ${mutation}`);
  }
  assert.equal((await rawRequest({ ...request, signatureHash: '00'.repeat(32) }, response.swapAuth)).status, 422);
  if (pendingHoldHash) {
    assert.equal((await rawRequest(request, response.swapAuth)).status, 422, 'a pending Lightning payment must refuse cooperation');
    await call('failHold', { hash: pendingHoldHash });
    await waitStatus(response.id, 'invoice.failedToPay');
    report.pendingPaymentRejected = true;
  }
  const partial = await getRgbRefundPartialSig(client, response.id, request, response.swapAuth);
  const replay = await getRgbRefundPartialSig(client, response.id, request, response.swapAuth);
  assert.deepEqual(replay, partial, 'same session must replay the stored public response');
  assert.equal((await rawRequest({ ...request, pubNonce: '02'.repeat(66) }, response.swapAuth)).status, 409);
  await persist(`${tag}-response`, partial);
  const finalized = session.complete(partial, key.secretKey);
  assert.throws(() => session.complete(partial, key.secretKey), /consumed|completed|session/i);
  const signed = await call('signWallet', { psbt: finalized.psbt, swapInputIndex: finalized.swapInputIndex });
  const inspect = await call('inspect', { hex: signed.hex });
  assert.equal(inspect.witnessLength, 1); assert.equal(inspect.locktime, 0);
  assert(inspect.hasRgbCommitment && inspect.feeSat > 0 && inspect.feeSat <= 5000);
  assert((await call('tip')) < Number(response.timeoutBlockHeight), 'early refund must precede CLTV');
  assert.equal((await call('mempool', { hex: signed.hex }))[0].allowed, true);
  await persist(`${tag}-signed`, { ...signed, operationId: color.operationId });
  await call('broadcast', { ...signed, operationId: color.operationId });
  const after = await call('balance', { expected: before.rgb });
  assert.equal(after.rgb, before.rgb, 'all RGB units must return exactly');
  await waitStatus(response.id, 'transaction.refunded');
  assert.deepEqual(await getRgbRefundPartialSig(client, response.id, request, response.swapAuth), partial);
  report.swapId = response.id; report.amount = Number(response.rgb.amount); report.beforeRgb = before.rgb; report.afterRgb = after.rgb;
  report.refund = inspect; report.proofTamperingRejected = true; report.authRejected = true; report.responseReplay = true; report.clientSessionConsumed = true;
  await writeFile(`${directory}coop-refund-validation-report.json`, `${JSON.stringify(report, null, 2)}\n`);
  console.log('PASS: real RGB cooperative refund before CLTV; exact units restored, proof/auth rejection, cached response, one-use session and confirmed status');
  session.free(); spend.free(); script.free();
} finally {
  child.stdin.end();
  await writeFile(`${directory}run/ts-coop-diagnostics.log`, diagnostics.join(''), { mode: 0o600 });
  client.free();
}
