/** Glue the examples share. None of it is part of the SDK. */
import {
  isKaleidoSwapError,
  type SwapClient,
  type Network,
} from "@kaleidorg/swap-sdk";

/** The maker's signet deployment settles on Mutinynet, so chain access must too. */
export const SIGNET_BITCOIN_ESPLORA = "https://esplora.signet.kaleidoswap.com";
export const SIGNET_LIQUID_ESPLORA =
  "https://blockstream.info/liquidtestnet/api";

export function network(): Network {
  const value = process.env.KALEIDO_NETWORK ?? "signet";
  if (value !== "signet" && value !== "regtest") {
    throw new Error(
      `KALEIDO_NETWORK=${value}: these examples run against "signet" (the live KaleidoSwap maker) or "regtest" (this repo's harness). ` +
        `Mainnet has no maker yet and forNetwork rejects it.`,
    );
  }
  return value;
}

/** Recorded beside each swap: a later refund must reach the maker that issued it. */
export function makerUrl(net: Network): string {
  return (
    process.env.KALEIDO_MAKER_URL ??
    (net === "signet"
      ? "https://maker.signet.kaleidoswap.com/v2"
      : "http://localhost:9001/v2")
  );
}

export function required(name: string, hint: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set — ${hint}`);
  return value;
}

export function optionalInt(name: string, fallback: number): number {
  const raw = process.env[name];
  if (!raw) return fallback;
  const parsed = Number(raw);
  if (!Number.isInteger(parsed))
    throw new Error(`${name}=${raw} is not an integer`);
  return parsed;
}

/** BIP85 child index. Reusing one reuses claim and refund material. */
export function swapIndex(): bigint {
  return BigInt(optionalInt("KALEIDO_SWAP_INDEX", 0));
}

/** Terminal non-success states; without them a failure polls until the timeout. */
const FAILURE_STATES = new Set([
  "swap.expired",
  "transaction.lockupFailed",
  "invoice.failedToPay",
  "invoice.expired",
  "transaction.failed",
  "transaction.refunded",
]);

/** Polls so the examples need no cleanup; production wants `SwapWsApi`. */
export async function waitForStatus(
  client: SwapClient,
  swapId: string,
  target: string,
): Promise<void> {
  const deadlineMs =
    Date.now() + optionalInt("KALEIDO_WAIT_TIMEOUT_SECS", 3600) * 1000;
  let previous = "";

  for (;;) {
    const { status } = await client.swap(swapId);
    if (status !== previous) {
      console.log(`  status: ${status}`);
      previous = status;
    }
    if (status === target) return;
    if (FAILURE_STATES.has(status)) {
      throw new Error(
        `swap ${swapId} entered terminal failure status ${status}`,
      );
    }
    if (Date.now() >= deadlineMs) {
      throw new Error(
        `timed out waiting for ${target}; last status was ${previous || "(none)"}`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 2000));
  }
}

export function die(error: unknown): never {
  if (isKaleidoSwapError(error)) {
    console.error(`\n${error.code}: ${error.message}`);
  } else {
    console.error(
      `\n${error instanceof Error ? error.message : String(error)}`,
    );
  }
  process.exit(1);
}
