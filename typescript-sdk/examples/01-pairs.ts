/**
 * What the maker will trade right now, and on what terms.
 * No keys, no funds, no configuration — run it first.
 *
 *   node examples/01-pairs.ts
 */
import { SwapClient, init } from "@kaleidorg/swap-sdk";

import { die, network } from "./common.ts";

type Limits = { minimal: bigint; maximal: bigint };
type Card = {
  hash: string;
  rate: number;
  limits: Limits;
  fees: {
    percentage: number;
    minerFees: bigint | { lockup: bigint; claim: bigint };
  };
};

/** `from -> to -> card`. The JS boundary hands back the map itself, not a wrapper. */
type Catalogue = Record<string, Record<string, Card>>;

function minerFeeTotal(fees: Card["fees"]): bigint {
  return typeof fees.minerFees === "object"
    ? fees.minerFees.lockup + fees.minerFees.claim
    : fees.minerFees;
}

function print(title: string, catalogue: Catalogue): void {
  console.log(`\n${title}`);
  const rows = Object.entries(catalogue).flatMap(([from, tos]) =>
    Object.entries(tos).map(([to, card]) => ({
      route: `${from} -> ${to}`,
      from,
      card,
    })),
  );

  if (rows.length === 0) {
    console.log("  (none advertised)");
    return;
  }

  for (const { route, from, card } of rows) {
    // Atomic units of the asset you send — sats for BTC and L-BTC, not L-USDT.
    const unit = from === "BTC" || from === "L-BTC" ? "sat" : `${from} units`;
    console.log(
      `  ${route.padEnd(16)} ` +
        `${card.limits.minimal.toLocaleString()}–${card.limits.maximal.toLocaleString()} ${unit}  ` +
        `fee ${card.fees.percentage}% + ${minerFeeTotal(card.fees)} ${unit}  ` +
        `hash ${card.hash.slice(0, 12)}…`,
    );
  }
}

try {
  await init();

  const client = SwapClient.forNetwork(network());

  const [submarine, reverse] = await Promise.all([
    client.submarinePairs() as Promise<Catalogue>,
    client.reversePairs() as Promise<Catalogue>,
  ]);

  print(
    "SUBMARINE  (you send the first leg, the maker pays a Lightning invoice)",
    submarine,
  );
  print(
    "REVERSE    (you pay a Lightning invoice, the maker sends the second leg)",
    reverse,
  );

  console.log(
    "\nARKD is quoted over the Intents corridor, not here — see 04-corridor.ts.",
  );
} catch (error) {
  die(error);
}
