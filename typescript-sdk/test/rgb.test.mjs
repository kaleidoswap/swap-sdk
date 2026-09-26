import assert from "node:assert/strict";
import test from "node:test";

import {
  init,
  RgbHtlcSpend,
  RgbSwaps,
  SwapClient,
  USDT_RGB,
  rgbCheckRecipientScript,
} from "../dist/index.node.js";

await init();

// A reverse swap whose swap tree, lock and lock transaction are consistent,
// generated from the Rust SDK's own test helpers (keys 0x01…/0x02…, preimage
// 0x07…). Nothing here reaches the network.
const FIXTURE = {
  dest: "51200303030303030303030303030303030303030303030303030303030303030303",
  lockTxHex:
    "02000000010000000000000000000000000000000000000000000000000000000000000000ffffffff00fdffffff01f605000000000000225120a8bcb0e66bd1d2df008a6c504ba26a98a59c982dcd93cb35b191c2d7a53cd8c900000000",
  preimage: "0707070707070707070707070707070707070707070707070707070707070707",
  response: {
    id: "SWAP",
    invoice: null,
    lockupAddress:
      "bcrt1p4z7tpent68fd7qy2d3gyhgn2nzjeexpdekfukdd3j8pd0ffumryslwt9nk",
    onchainAmount: 1000000,
    refundPublicKey:
      "024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766",
    rgb: {
      amount: 1000000,
      assetId: "rgb:2dkSTbr-jFhznbPmo-TQafzswCN-av4gTsJjX-ttx6CNou5-M98k8Zd",
      blinding: 42,
      claimFeeRate: 5,
      htlcSat: 1526,
      minConfirmations: 1,
      recipientId: "bcrt:wvout:htlc",
      scriptPubkey:
        "5120a8bcb0e66bd1d2df008a6c504ba26a98a59c982dcd93cb35b191c2d7a53cd8c9",
      transportEndpoints: ["rpcs://proxy.example/json-rpc"],
    },
    swapTree: {
      claimLeaf: {
        output:
          "82012088a914b566a3eecce809896361988823cd2f423fe800e788201b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078fac",
        version: 192,
      },
      refundLeaf: {
        output:
          "204d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766ad02f401b1",
        version: 192,
      },
    },
    timeoutBlockHeight: 500,
  },
  takerPubkey:
    "031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f",
  takerSecret:
    "0101010101010101010101010101010101010101010101010101010101010101",
};

const { response, lockTxHex, takerSecret, takerPubkey, preimage, dest } =
  FIXTURE;

function withCode(code, pattern) {
  return (error) => {
    assert.equal(error.code, code, `${error.code}: ${error.message}`);
    if (pattern) assert.match(error.message, pattern);
    return true;
  };
}

/** Read a Bitcoin compact size at `at`: [value, width]. */
function compactSize(bytes, at) {
  const first = bytes[at];
  if (first < 0xfd) return [first, 1];
  if (first === 0xfd) return [bytes[at + 1] | (bytes[at + 2] << 8), 3];
  throw new Error("PSBT value too large for this test helper");
}

function encodeCompactSize(n) {
  if (n < 0xfd) return Buffer.from([n]);
  return Buffer.from([0xfd, n & 0xff, n >> 8]);
}

/**
 * What rgb-lib's `psbt_op_prepare` does to the transaction: write a 32-byte
 * commitment into the empty OP_RETURN at output 0.
 */
function color(psbtBase64) {
  const bytes = Buffer.from(psbtBase64, "base64");
  // magic (5) | keylen 0x01 | key 0x00 (unsigned tx) | value
  assert.equal(bytes.subarray(0, 7).toString("hex"), "70736274ff0100");
  const [txLen, width] = compactSize(bytes, 7);
  const txStart = 7 + width;
  const tx = bytes.subarray(txStart, txStart + txLen);
  const empty = Buffer.from("0000000000000000026a00", "hex");
  const at = tx.indexOf(empty);
  assert.ok(at > 0, "the spend has an empty OP_RETURN");
  const committed = Buffer.concat([
    Buffer.alloc(8),
    Buffer.from([0x22, 0x6a, 0x20]),
    Buffer.alloc(32, 0xab),
  ]);
  const colored = Buffer.concat([
    tx.subarray(0, at),
    committed,
    tx.subarray(at + empty.length),
  ]);
  return Buffer.concat([
    bytes.subarray(0, 7),
    encodeCompactSize(colored.length),
    colored,
    bytes.subarray(txStart + txLen),
  ]).toString("base64");
}

test("the taker claims a reverse lock through the colored spend", () => {
  const spend = RgbHtlcSpend.claim(
    "regtest",
    response,
    takerPubkey,
    lockTxHex,
    dest,
  );
  assert.equal(typeof spend.feeSat(), "bigint");
  assert.equal(spend.feeSat() + 546n, BigInt(response.rgb.htlcSat));
  const psbt = spend.psbt();

  // Uncolored, the spend would burn the asset: refused.
  assert.throws(
    () => spend.signColoredTx(psbt, takerSecret, preimage),
    withCode("Protocol", /burn the asset/),
  );
  // A claim needs the preimage.
  assert.throws(
    () => spend.signColoredTx(color(psbt), takerSecret),
    withCode("Protocol", /preimage/),
  );

  const tx = spend.signColoredTx(color(psbt), takerSecret, preimage);
  const hex = tx.hex();
  assert.match(hex, new RegExp(preimage), "the witness reveals the preimage");
  assert.match(hex, new RegExp(response.swapTree.claimLeaf.output));
  assert.match(hex, /6a20(ab){32}/, "the commitment is kept");
  assert.equal(tx.txid().length, 64);

  // With a fee input the wallet signs later: the PSBT form.
  const signed = spend.signColored(color(psbt), takerSecret, preimage);
  assert.notEqual(signed, color(psbt));
  spend.free();
});

test("a claim is refused for another key or another lock transaction", () => {
  const spend = RgbHtlcSpend.claim(
    "regtest",
    response,
    takerPubkey,
    lockTxHex,
    dest,
  );
  assert.throws(
    () => spend.signColoredTx(color(spend.psbt()), "02".repeat(32), preimage),
    withCode("Protocol", /keys/),
  );
  // A transaction that does not pay the HTLC.
  const stranger = lockTxHex.replace(response.rgb.scriptPubkey, dest);
  assert.throws(
    () => RgbHtlcSpend.claim("regtest", response, takerPubkey, stranger, dest),
    withCode("Protocol", /no output paying the HTLC/),
  );
  assert.throws(
    () => RgbHtlcSpend.claim("regtest", response, takerPubkey, "zz", dest),
    withCode("InvalidArgument", /lockTxHex/),
  );
});

test("the recipient id's script must be the HTLC", () => {
  rgbCheckRecipientScript(response.rgb, response.rgb.scriptPubkey);
  assert.throws(
    () => rgbCheckRecipientScript(response.rgb, dest),
    withCode("Protocol", /recipientId/),
  );
});

test("RGB routes have their own create methods", async () => {
  const client = SwapClient.forNetwork("regtest");
  // The generic create path points at the RGB one instead of guessing.
  await assert.rejects(
    client.createSubmarineSwap("regtest", {
      from: USDT_RGB,
      to: "BTC",
      invoice: "lnbcrt1",
      refundPublicKey: takerPubkey,
    }),
    withCode("InvalidArgument", /createRgbSubmarineSwap/),
  );
  // And the RGB path refuses a non-RGB route before any request.
  const rgb = new RgbSwaps(client);
  await assert.rejects(
    rgb.createSubmarineSwap("regtest", {
      from: "BTC",
      to: "BTC",
      invoice: "lnbcrt1",
      refundPublicKey: takerPubkey,
    }),
    withCode("Protocol", /sends USDT-RGB/),
  );
  await assert.rejects(
    rgb.createReverseSwap("regtest", {
      from: "BTC",
      to: "L-BTC",
      claimPublicKey: takerPubkey,
      preimageHash: "00".repeat(32),
      invoiceAmount: 1000n,
    }),
    withCode("Protocol", /receives USDT-RGB/),
  );
});

test("atomic steps refuse a quote whose offer is not JSON", async () => {
  const rgb = new RgbSwaps(SwapClient.forNetwork("regtest"));
  const quote = {
    id: "01J9ATOMIC",
    pair: "BTC/USDT-RGB",
    direction: "from",
    fromAmount: 100000n,
    toAmount: 60000000n,
    assetId: response.rgb.assetId,
    networkFeeSat: 700n,
    serviceFee: 1n,
    expiresAt: 1n,
    offerExpiresAt: 2n,
    offerJson: "{not json",
  };
  await assert.rejects(
    rgb.atomicRequest(quote, "{}"),
    withCode("InvalidArgument", /offerJson/),
  );
  await assert.rejects(
    rgb.atomicComplete({ ...quote, offerJson: "{}" }, "nope"),
    withCode("InvalidArgument", /completionJson/),
  );
});
