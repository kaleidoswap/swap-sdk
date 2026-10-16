import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import test from "node:test";
import {
  ATOMIC_BTC_TO_USDT_RGB,
  AtomicSwaps,
  init,
  SwapClient,
} from "../dist/index.node.js";

await init();

const ASSET = "rgb:test-usdt-contract";
const NOW = 1_800_000_000n;
const TXID = "aa".repeat(32);
// Above 2^53: a seal blinding as rgb-lib writes it. A JS number cannot hold it.
const BLINDING = "18446744073709551557";

// rgb-lib messages as raw JSON text, the way the wallet's rgb-lib binding
// produces them. `BLINDING` is written as a bare JSON number.
const OFFER_JSON = `{"swap_id":"rgbswap1","maker_gives":{"kind":"Rgb","asset_id":"${ASSET}","amount":95000000},"maker_receives":{"kind":"Btc","asset_id":null,"amount":100000},"bitcoin_network":"Regtest","network_fee_sat":1200,"rgb_output_sat":1000,"expiration_timestamp":${NOW + 300n},"maker_btc_address":"bcrt1qmaker","proxy_url":"rpcs://proxy.example/json-rpc","platform_fee_sat":0,"fee_recipient_script_pubkey_hex":null}`;
const REQUEST_JSON = `{"offer":${OFFER_JSON},"taker_inputs":[],"taker_rgb_blinding":${BLINDING},"taker_change_script_pubkey_hex":"0014"}`;
const PROPOSAL_JSON = `{"request":${REQUEST_JSON},"maker_inputs":[],"txid":"${TXID}","psbt":"cHNidP8="}`;
const COMPLETION_JSON = `{"proposal":${PROPOSAL_JSON},"txid":"${TXID}","psbt":"cHNidP8="}`;

const QUOTE_BODY = `{"id":"01JATOMICSWAP","pair":"BTC/USDT-RGB","direction":"from","fromAmount":100000,"toAmount":95000000,"assetId":"${ASSET}","networkFeeSat":1200,"serviceFee":500000,"expiresAt":${NOW + 60n},"offerExpiresAt":${NOW + 300n},"offer":${OFFER_JSON}}`;

function statusBody(status) {
  return `{"id":"01JATOMICSWAP","pair":"BTC/USDT-RGB","status":"${status}","state":"x","fromAmount":100000,"toAmount":95000000,"assetId":"${ASSET}","networkFeeSat":1200,"txid":"${TXID}","confirmations":0,"expiresAt":${NOW + 60n},"offerExpiresAt":${NOW + 300n},"createdAt":${NOW}}`;
}

const EXPECT = { assetId: ASSET, bitcoinNetwork: "Regtest" };

/** A maker answering each request with the next raw JSON body. */
async function mockMaker(bodies) {
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      seen.push({ method: req.method, url: req.url, body });
      const [status, text] = bodies.shift() ?? [500, '{"error":"unexpected"}'];
      res.writeHead(status, { "content-type": "application/json" });
      res.end(text);
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${server.address().port}/v2`;
  return { url, seen, close: () => server.close() };
}

function hasCode(code, message) {
  return (error) => {
    assert.ok(error instanceof Error);
    assert.equal(error.code, code);
    assert.match(error.message, message);
    return true;
  };
}

test("an atomic swap runs quote, request, complete and track, with rgb-lib JSON intact", async () => {
  const maker = await mockMaker([
    [201, QUOTE_BODY],
    [
      200,
      `{"id":"01JATOMICSWAP","status":"transaction.proposed","proposal":${PROPOSAL_JSON}}`,
    ],
    [
      200,
      `{"id":"01JATOMICSWAP","status":"transaction.mempool","txid":"${TXID}","completion":${COMPLETION_JSON}}`,
    ],
    [200, statusBody("transaction.mempool")],
    [200, statusBody("transaction.claimed")],
  ]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    const quote = await atomic.quote(
      { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n },
      { ...EXPECT, maxNetworkFeeSat: 1_200n },
      NOW,
    );
    assert.equal(quote.toAmount, 95_000_000n);
    assert.equal(typeof quote.offer, "string");
    assert.deepEqual(JSON.parse(quote.offer), JSON.parse(OFFER_JSON));
    assert.deepEqual(JSON.parse(maker.seen[0].body), {
      pair: "BTC/USDT-RGB",
      amount: 100000,
      direction: "from",
    });

    const proposed = await atomic.request(quote.id, REQUEST_JSON, []);
    // The 64-bit blinding reaches the maker and comes back digit for digit.
    assert.ok(maker.seen[1].body.includes(`"taker_rgb_blinding":${BLINDING}`));
    assert.ok(proposed.proposal.includes(`"taker_rgb_blinding":${BLINDING}`));
    assert.equal(maker.seen[1].url, "/v2/swap/atomic/01JATOMICSWAP/request");

    const done = await atomic.complete(quote.id, COMPLETION_JSON);
    assert.equal(done.txid, TXID);
    assert.equal(maker.seen[2].url, "/v2/swap/atomic/01JATOMICSWAP/complete");

    const seenStates = [];
    const final = await atomic.track(quote.id, {
      intervalMs: 1,
      onStatus: (s) => seenStates.push(s.status),
    });
    assert.equal(final.status, "transaction.claimed");
    assert.deepEqual(seenStates, [
      "transaction.mempool",
      "transaction.claimed",
    ]);
  } finally {
    maker.close();
  }
});

test("a quote whose offer differs from it never reaches the caller", async () => {
  const tampered = QUOTE_BODY.replace('"amount":95000000', '"amount":1');
  const maker = await mockMaker([[201, tampered]]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    await assert.rejects(
      atomic.quote(
        { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n },
        EXPECT,
        NOW,
      ),
      hasCode("Protocol", /offer gives/),
    );
  } finally {
    maker.close();
  }
});

test("a quote for another asset or past its deadline is refused", async () => {
  const maker = await mockMaker([
    [201, QUOTE_BODY],
    [201, QUOTE_BODY],
  ]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    const request = { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n };
    await assert.rejects(
      atomic.quote(request, { ...EXPECT, assetId: "rgb:other" }, NOW),
      hasCode("Protocol", /is for asset/),
    );
    await assert.rejects(
      atomic.quote(request, EXPECT, NOW + 60n),
      hasCode("Protocol", /expired/),
    );
  } finally {
    maker.close();
  }
});

test("a proposal for another request is refused", async () => {
  const other = PROPOSAL_JSON.replace(BLINDING, "1");
  const maker = await mockMaker([
    [
      200,
      `{"id":"01JATOMICSWAP","status":"transaction.proposed","proposal":${other}}`,
    ],
  ]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    await assert.rejects(
      atomic.request("01JATOMICSWAP", REQUEST_JSON, []),
      hasCode("Protocol", /not for the request that was sent/),
    );
  } finally {
    maker.close();
  }
});

test("rgb-lib messages must be JSON text", async () => {
  const atomic = new AtomicSwaps(new SwapClient("http://127.0.0.1:1/v2"));
  await assert.rejects(
    atomic.request("01JATOMICSWAP", "{not json", []),
    hasCode("InvalidArgument", /requestJson/),
  );
});

// A real maker's bodies (`tests/fixtures/atomic-v1`), replayed byte for byte.
const CAPTURE = JSON.parse(
  readFileSync(
    new URL(
      "../../tests/fixtures/atomic-v1/wire-capture.json",
      import.meta.url,
    ),
    "utf8",
  ),
).swaps;

for (const [pair, capture] of Object.entries(CAPTURE)) {
  test(`a real ${pair} swap replays through AtomicSwaps with every digit intact`, async () => {
    const maker = await mockMaker(
      ["quote", "request", "complete", "status"].map((name) => [
        200,
        capture[name],
      ]),
    );
    try {
      const atomic = new AtomicSwaps(new SwapClient(maker.url));
      const quote = await atomic.quote(
        { pair, amount: BigInt(JSON.parse(capture.quoteSent).amount) },
        {
          assetId: JSON.parse(capture.quote).assetId,
          bitcoinNetwork: "Regtest",
          maxNetworkFeeSat: 2_000n,
          maxOfferTtlSecs: 600n,
          maxRgbOutputSat: 1_000n,
        },
        BigInt(capture.now),
      );
      // rgb-lib's request and completion, as the wallet wrote them.
      const request = capture.requestSent.slice('{"request":'.length, -1);
      const completion = capture.completeSent.slice(
        '{"completion":'.length,
        -1,
      );
      // The taker's wallet: the request's own inputs and a coin it did not
      // spend.
      const wallet = [
        ...JSON.parse(request).taker_inputs.map(({ outpoint }) => outpoint),
        { txid: "ee".repeat(32), vout: 7 },
      ];
      const proposed = await atomic.request(quote.id, request, wallet);
      // The maker's coins, for a wallet on an unpatched rgb-lib to check.
      const makers = JSON.parse(proposed.proposal).maker_inputs;
      assert.ok(makers.length > 0);
      assert.deepEqual(
        proposed.makerInputs,
        makers.map(({ outpoint }) => ({
          txid: outpoint.txid,
          vout: outpoint.vout,
        })),
      );
      const done = await atomic.complete(quote.id, completion);
      const status = await atomic.status(quote.id);

      assert.equal(status.status, "transaction.claimed");
      assert.equal(status.txid, done.txid);
      assert.deepEqual(
        maker.seen.map((r) => `${r.method} ${r.url}`),
        [
          "POST /v2/swap/atomic/quote",
          `POST /v2/swap/atomic/${quote.id}/request`,
          `POST /v2/swap/atomic/${quote.id}/complete`,
          `GET /v2/swap/atomic/${quote.id}`,
        ],
      );
      // Every blinding above 2^53 reaches the maker and comes back as written.
      const blindings = request.match(/"(?:taker|maker)_rgb_blinding":\d+/g);
      assert.ok(blindings.length > 0);
      for (const blinding of blindings) {
        assert.ok(BigInt(blinding.split(":")[1]) > 2n ** 53n);
        assert.ok(maker.seen[1].body.includes(blinding), blinding);
        assert.ok(proposed.proposal.includes(blinding), blinding);
      }
      assert.ok(done.completion.includes(blindings[0]));
    } finally {
      maker.close();
    }
  });
}

test("a proposal listing any of the taker's coins as the maker's is refused", async () => {
  // The request spends aa:0; the wallet also holds dd:5, which the request
  // does not touch. The maker lists dd:5 as its own.
  const spent = { txid: "aa".repeat(32), vout: 0 };
  const other = { txid: "dd".repeat(32), vout: 5 };
  const request = REQUEST_JSON.replace(
    '"taker_inputs":[]',
    `"taker_inputs":[{"outpoint":${JSON.stringify(spent)},"amount_sat":5000}]`,
  );
  const proposal = PROPOSAL_JSON.replace(REQUEST_JSON, request).replace(
    '"maker_inputs":[]',
    `"maker_inputs":[{"outpoint":${JSON.stringify(other)},"amount_sat":90000}]`,
  );
  const maker = await mockMaker([
    [
      200,
      `{"id":"01JATOMICSWAP","status":"transaction.proposed","proposal":${proposal}}`,
    ],
  ]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    await assert.rejects(
      atomic.request("01JATOMICSWAP", request, [spent, other]),
      hasCode("Protocol", /taker's own coin d+:5/),
    );
    // Without the whole wallet the SDK cannot tell; it refuses a list that
    // lacks the request's own inputs before posting anything.
    for (const wallet of [[], [other]]) {
      await assert.rejects(
        atomic.request("01JATOMICSWAP", request, wallet),
        hasCode("InvalidArgument", /list_unspents/),
      );
    }
    await assert.rejects(
      atomic.request("01JATOMICSWAP", request, undefined),
      hasCode("InvalidArgument", /walletOutpoints/),
    );
    assert.equal(
      maker.seen.length,
      1,
      "only the full-wallet request was posted",
    );
  } finally {
    maker.close();
  }
});

test("an offer whose RGB output exceeds maxRgbOutputSat is refused", async () => {
  const maker = await mockMaker([[201, QUOTE_BODY]]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    await assert.rejects(
      atomic.quote(
        { pair: ATOMIC_BTC_TO_USDT_RGB, amount: 100_000n },
        { ...EXPECT, maxRgbOutputSat: 999n },
        NOW,
      ),
      hasCode("Protocol", /RGB output/),
    );
  } finally {
    maker.close();
  }
});

test("a taker's coin respelled in uppercase hex is still refused", async () => {
  const spent = { txid: "aa".repeat(32), vout: 0 };
  const other = { txid: "dd".repeat(32), vout: 5 };
  const request = REQUEST_JSON.replace(
    '"taker_inputs":[]',
    `"taker_inputs":[{"outpoint":${JSON.stringify(spent)},"amount_sat":5000}]`,
  );
  const respelled = { txid: "DD".repeat(32), vout: 5 };
  const proposal = PROPOSAL_JSON.replace(REQUEST_JSON, request).replace(
    '"maker_inputs":[]',
    `"maker_inputs":[{"outpoint":${JSON.stringify(respelled)},"amount_sat":90000}]`,
  );
  const maker = await mockMaker([
    [
      200,
      `{"id":"01JATOMICSWAP","status":"transaction.proposed","proposal":${proposal}}`,
    ],
  ]);
  try {
    const atomic = new AtomicSwaps(new SwapClient(maker.url));
    await assert.rejects(
      atomic.request("01JATOMICSWAP", request, [spent, other]),
      hasCode("Protocol", /taker's own coin/),
    );
  } finally {
    maker.close();
  }
});
