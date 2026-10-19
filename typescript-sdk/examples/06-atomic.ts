/**
 * Atomic BTC <-> USDT-RGB swap on L1: one transaction moves both legs, with no
 * HTLC and no refund. The SDK quotes, verifies and tracks; the two signing
 * steps belong to an rgb-lib wallet, so this example hands them over in files:
 *
 *   node examples/06-atomic.ts
 *     Quote and verify. Writes the offer to $ATOMIC_DIR/offer.json and stops.
 *     Feed it to the wallet's accept_swap_offer; save the request.
 *
 *   ATOMIC_SWAP_ID=... node examples/06-atomic.ts request
 *     Posts $ATOMIC_DIR/request.json, writes proposal.json. Needs the
 *     wallet's list_unspents output in unspents.json: a proposal that lists
 *     any of those coins as the maker's is refused. Feed proposal.json to
 *     complete_swap_proposal; save the completion.
 *
 *   ATOMIC_SWAP_ID=... node examples/06-atomic.ts complete
 *     Posts $ATOMIC_DIR/completion.json; the maker signs last and broadcasts.
 *     Writes the finalized completion for accept_swap_transfers, then tracks
 *     the swap to a terminal state.
 *
 * The live signet maker ships atomic pairs disabled; run it against a regtest
 * maker with KALEIDO_NETWORK=regtest.
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import {
  ATOMIC_BTC_TO_USDT_RGB,
  ATOMIC_USDT_RGB_TO_BTC,
  AtomicSwaps,
  SwapClient,
  init,
  type AtomicPairName,
} from "@kaleidorg/swap-sdk";

import { die, makerUrl, network, optionalInt, required } from "./common.ts";

try {
  await init();

  const net = network();
  const atomic = new AtomicSwaps(new SwapClient(makerUrl(net)));
  const dir = process.env.ATOMIC_DIR ?? "atomic-swap";
  await mkdir(dir, { recursive: true });
  const step = process.argv[2] ?? "quote";

  if (step === "quote") {
    const pair: AtomicPairName =
      process.env.ATOMIC_PAIR === ATOMIC_USDT_RGB_TO_BTC
        ? ATOMIC_USDT_RGB_TO_BTC
        : ATOMIC_BTC_TO_USDT_RGB;
    const cards = await atomic.pairs();
    const [from, to] = pair.split("/");
    const card = cards[from]?.[to];
    if (!card) {
      throw new Error(
        `the maker serves no ${pair} atomic pair (it lists ${JSON.stringify(Object.keys(cards))})`,
      );
    }
    console.log(
      `${pair}: rate ${card.rate}, ${card.fees.percentage}% fee, ` +
        `limits ${card.limits.minimal}..${card.limits.maximal}`,
    );

    // Throws unless the quote, and the offer inside it, are what was asked.
    const quote = await atomic.quote(
      {
        pair,
        amount: BigInt(optionalInt("ATOMIC_AMOUNT", 100_000)),
        direction: "from",
      },
      {
        assetId: required(
          "USDT_RGB_ASSET_ID",
          "the rgb:... contract id your wallet holds",
        ),
        // rgb-lib's network name for the wallet; check yours for signet.
        bitcoinNetwork:
          process.env.RGB_BITCOIN_NETWORK ??
          (net === "regtest" ? "Regtest" : "Signet"),
        maxNetworkFeeSat: BigInt(optionalInt("ATOMIC_MAX_FEE_SAT", 10_000)),
        // The price protection: the least you accept to receive, in the
        // output asset's base units (sats, or USDT-RGB's 6-decimal units).
        // maxServiceFee alone would not bound it.
        minToAmount: BigInt(
          required(
            "ATOMIC_MIN_TO_AMOUNT",
            "the least you accept to receive, in base units",
          ),
        ),
        maxOfferTtlSecs: 3_600n,
        // Sats each RGB receive output carries; rgb-lib's default is 1,000.
        maxRgbOutputSat: 2_000n,
      },
    );
    await writeFile(join(dir, "offer.json"), quote.offer);
    console.log(`quote ${quote.id}`);
    console.log(`  you send:    ${quote.fromAmount}`);
    console.log(`  you receive: ${quote.toAmount} (fee ${quote.serviceFee})`);
    console.log(`  miner fee:   ${quote.networkFeeSat} sat, on top`);
    console.log(
      `  request by:  ${new Date(Number(quote.expiresAt) * 1000).toISOString()}`,
    );
    console.log(
      `\noffer written to ${join(dir, "offer.json")}. Run accept_swap_offer, save ` +
        `request.json beside it, then:\n  ATOMIC_SWAP_ID=${quote.id} node 06-atomic.ts request`,
    );
  } else if (step === "request") {
    const id = required("ATOMIC_SWAP_ID", "the quote id");
    const request = await readFile(join(dir, "request.json"), "utf8");
    // Every coin the wallet holds: rgb-lib's list_unspents. rgb-lib before
    // kaleidoswap/rgb-lib#18 signs any of them a proposal lists, so the SDK
    // refuses a proposal that lists one as the maker's.
    const unspents: { utxo: { outpoint: { txid: string; vout: number } } }[] =
      JSON.parse(await readFile(join(dir, "unspents.json"), "utf8"));
    const { proposal, status, makerInputs } = await atomic.request(
      id,
      request,
      unspents.map(({ utxo }) => utxo.outpoint),
    );
    await writeFile(join(dir, "proposal.json"), proposal);
    console.log(`maker inputs (none of them yours):`);
    for (const { txid, vout } of makerInputs) console.log(`  ${txid}:${vout}`);
    console.log(
      `${id}: ${status}. Run complete_swap_proposal on proposal.json,`,
    );
    console.log(`save completion.json, then: node 06-atomic.ts complete`);
  } else if (step === "complete") {
    const id = required("ATOMIC_SWAP_ID", "the quote id");
    const completion = await readFile(join(dir, "completion.json"), "utf8");
    const done = await atomic.complete(id, completion);
    await writeFile(join(dir, "finalized.json"), done.completion);
    console.log(`${id}: broadcast ${done.txid}`);
    const final = await atomic.track(id, {
      onStatus: (s) =>
        console.log(`  ${s.status} (${s.confirmations} confirmations)`),
    });
    console.log(
      final.status === "transaction.claimed"
        ? "settled. Run accept_swap_transfers with finalized.json."
        : `ended ${final.status}`,
    );
  } else {
    throw new Error(`unknown step ${step}: quote | request | complete`);
  }
} catch (error) {
  die(error);
}
