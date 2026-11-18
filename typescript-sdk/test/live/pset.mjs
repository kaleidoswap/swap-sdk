// Just enough PSET (Elements PSBT v2) to fund an SDK claim or refund template
// with one explicit policy-asset input: parse the key-value maps, append an
// input and outputs, serialize. Signing is the elementsd wallet's job.

const MAGIC = Buffer.from("pset\xff", "latin1");

// BIP 370 keys.
const GLOBAL_INPUT_COUNT = 0x04;
const GLOBAL_OUTPUT_COUNT = 0x05;
const IN_WITNESS_UTXO = 0x01;
const IN_PREVIOUS_TXID = 0x0e;
const IN_OUTPUT_INDEX = 0x0f;
const IN_SEQUENCE = 0x10;
const OUT_AMOUNT = 0x03;
const OUT_SCRIPT = 0x04;
// Elements' proprietary output asset: 0xfc, "pset", subtype 0x02.
const ELEMENTS_OUT_ASSET = Buffer.from([
  0xfc,
  0x04,
  ...Buffer.from("pset"),
  0x02,
]);

function readCompact(buf, at) {
  const first = buf[at];
  if (first < 0xfd) return [first, at + 1];
  if (first === 0xfd) return [buf.readUInt16LE(at + 1), at + 3];
  if (first === 0xfe) return [buf.readUInt32LE(at + 1), at + 5];
  return [Number(buf.readBigUInt64LE(at + 1)), at + 9];
}

function compact(n) {
  if (n < 0xfd) return Buffer.from([n]);
  if (n <= 0xffff) {
    const b = Buffer.alloc(3);
    b[0] = 0xfd;
    b.writeUInt16LE(n, 1);
    return b;
  }
  const b = Buffer.alloc(5);
  b[0] = 0xfe;
  b.writeUInt32LE(n, 1);
  return b;
}

/** One map: `[{ key, value }]` in file order. */
function readMap(buf, at) {
  const map = [];
  for (;;) {
    let len;
    [len, at] = readCompact(buf, at);
    if (len === 0) return [map, at];
    const key = buf.subarray(at, at + len);
    at += len;
    [len, at] = readCompact(buf, at);
    map.push({ key, value: buf.subarray(at, at + len) });
    at += len;
  }
}

function writeMap(map) {
  return Buffer.concat([
    ...map.flatMap(({ key, value }) => [
      compact(key.length),
      key,
      compact(value.length),
      value,
    ]),
    Buffer.from([0]),
  ]);
}

const entry = (type, value) => ({ key: Buffer.from([type]), value });
const findKey = (map, type) =>
  map.find(({ key }) => key.length === 1 && key[0] === type);

function u32(n) {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n);
  return b;
}

function u64(n) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
}

/** An explicit Elements TxOut: asset, value, empty nonce, script. */
function explicitTxOut(assetHex, sats, scriptHex) {
  const value = Buffer.alloc(9);
  value[0] = 0x01;
  value.writeBigUInt64BE(BigInt(sats), 1);
  const script = Buffer.from(scriptHex, "hex");
  return Buffer.concat([
    Buffer.from([0x01]),
    Buffer.from(assetHex, "hex").reverse(),
    value,
    Buffer.from([0x00]),
    compact(script.length),
    script,
  ]);
}

export function parse(base64) {
  const buf = Buffer.from(base64, "base64");
  if (!buf.subarray(0, 5).equals(MAGIC)) throw new Error("not a PSET");
  let at = 5;
  let global;
  [global, at] = readMap(buf, at);
  const count = (type) => {
    const found = findKey(global, type);
    if (!found) throw new Error(`PSET global map lacks key ${type}`);
    return readCompact(found.value, 0)[0];
  };
  const inputs = [];
  const outputs = [];
  for (let i = count(GLOBAL_INPUT_COUNT); i > 0; i--) {
    let map;
    [map, at] = readMap(buf, at);
    inputs.push(map);
  }
  for (let i = count(GLOBAL_OUTPUT_COUNT); i > 0; i--) {
    let map;
    [map, at] = readMap(buf, at);
    outputs.push(map);
  }
  return { global, inputs, outputs };
}

export function serialize({ global, inputs, outputs }) {
  const counts = global.map((e) =>
    e.key.length === 1 && e.key[0] === GLOBAL_INPUT_COUNT
      ? entry(GLOBAL_INPUT_COUNT, compact(inputs.length))
      : e.key.length === 1 && e.key[0] === GLOBAL_OUTPUT_COUNT
        ? entry(GLOBAL_OUTPUT_COUNT, compact(outputs.length))
        : e,
  );
  return Buffer.concat([
    MAGIC,
    writeMap(counts),
    ...inputs.map(writeMap),
    ...outputs.map(writeMap),
  ]).toString("base64");
}

/**
 * Adds the explicit policy-asset UTXO `utxo` ({ txid, vout, sats, scriptHex })
 * as an input, `change` sats to `changeScriptHex` and an explicit `fee`.
 */
export function fundWithExplicitInput(
  base64,
  { policyAssetHex, utxo, changeScriptHex, fee },
) {
  const pset = parse(base64);
  pset.inputs.push([
    entry(IN_PREVIOUS_TXID, Buffer.from(utxo.txid, "hex").reverse()),
    entry(IN_OUTPUT_INDEX, u32(utxo.vout)),
    entry(IN_SEQUENCE, u32(0)),
    entry(
      IN_WITNESS_UTXO,
      explicitTxOut(policyAssetHex, utxo.sats, utxo.scriptHex),
    ),
  ]);
  const asset = Buffer.from(policyAssetHex, "hex").reverse();
  const output = (sats, scriptHex) => [
    entry(OUT_AMOUNT, u64(sats)),
    entry(OUT_SCRIPT, Buffer.from(scriptHex, "hex")),
    { key: ELEMENTS_OUT_ASSET, value: asset },
  ];
  pset.outputs.push(
    output(BigInt(utxo.sats) - BigInt(fee), changeScriptHex),
    output(fee, ""),
  );
  return serialize(pset);
}
