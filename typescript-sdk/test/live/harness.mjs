// The live suite's view of a regtest maker: where everything is, and the
// chain and Lightning moves the tests make around the SDK. See README.md.
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";

import { SwapClient, SwapMasterKey, init } from "../../dist/index.node.js";
import { LdkServer } from "./ldk.mjs";

/** `false` when the suite should run, or why it is skipped. */
export const skip =
  process.env.KALEIDO_LIVE === "1"
    ? false
    : "live suite: set KALEIDO_LIVE=1 and the KALEIDO_LIVE_* variables (test/live/README.md)";

function env(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set (test/live/README.md)`);
  return value;
}

export const NETWORK = "regtest";

/** Built lazily so a skipped suite needs no configuration. */
export async function live() {
  await init();
  const makerUrl = env("KALEIDO_LIVE_MAKER_URL");
  return {
    makerUrl,
    wsUrl:
      process.env.KALEIDO_LIVE_WS_URL ??
      `${makerUrl.replace(/^http/, "ws")}/ws`,
    client: new SwapClient(makerUrl),
    btcEsplora: env("KALEIDO_LIVE_BTC_ESPLORA"),
    liquidEsplora: env("KALEIDO_LIVE_LIQUID_ESPLORA"),
    btc: new Node(env("KALEIDO_LIVE_BITCOIND_RPC"), "bitcoin"),
    liquid: new Node(env("KALEIDO_LIVE_ELEMENTSD_RPC"), "liquid"),
    ln: new LdkServer(
      env("KALEIDO_LIVE_LDK_GRPC"),
      env("KALEIDO_LIVE_LDK_API_KEY"),
      readFileSync(env("KALEIDO_LIVE_LDK_CERT")),
    ),
  };
}

/**
 * A swap master key no earlier run has used: the BIP39 test vector with a
 * random passphrase. Neither half is printed, and nothing outlives the run.
 */
export function freshMaster() {
  return SwapMasterKey.fromWalletMnemonic(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    NETWORK,
    randomBytes(16).toString("hex"),
  );
}

/** A bitcoind or elementsd wallet over JSON-RPC (`http://user:pass@host:port/wallet/<name>`). */
export class Node {
  constructor(url, chain) {
    const parsed = new URL(url);
    this.auth = `Basic ${Buffer.from(
      `${decodeURIComponent(parsed.username)}:${decodeURIComponent(parsed.password)}`,
    ).toString("base64")}`;
    parsed.username = "";
    parsed.password = "";
    this.url = parsed.toString();
    this.chain = chain;
  }

  async rpc(method, ...params) {
    const response = await fetch(this.url, {
      method: "POST",
      headers: { authorization: this.auth, "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "1.0", id: method, method, params }),
    });
    const { result, error } = await response.json();
    if (error) throw new Error(`${this.chain} ${method}: ${error.message}`);
    return result;
  }

  newAddress() {
    return this.rpc("getnewaddress");
  }

  /** Pays `sats` to `address`; resolves to the txid. */
  send(address, sats) {
    return this.rpc("sendtoaddress", address, Number(sats) / 1e8);
  }

  async mine(blocks = 1) {
    return this.rpc("generatetoaddress", blocks, await this.newAddress());
  }

  height() {
    return this.rpc("getblockcount");
  }

  rawTransaction(txid) {
    return this.rpc("getrawtransaction", txid);
  }

  /** Whether `txid` is in the mempool or a block. */
  async knows(txid) {
    try {
      await this.rawTransaction(txid);
      return true;
    } catch {
      return false;
    }
  }
}

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Polls `probe` until it returns something truthy. */
export async function until(
  what,
  probe,
  { timeoutMs = 120_000, everyMs = 1_000 } = {},
) {
  const deadline = Date.now() + timeoutMs;
  let last;
  for (;;) {
    last = await probe();
    if (last) return last;
    if (Date.now() > deadline) {
      throw new Error(
        `timed out waiting for ${typeof what === "function" ? what() : what}`,
      );
    }
    await sleep(everyMs);
  }
}

/**
 * Waits for one of `statuses`. With `mine`, mines one block on that node when
 * the status has not moved for a while, so lockups confirm. Never faster:
 * Lightning nodes that see a tip moving under an HTLC refuse it as
 * `IncorrectPaymentDetails`. Throws as soon as the swap reaches `failOn`.
 */
export async function waitStatus(
  client,
  swapId,
  statuses,
  { mine, failOn = [], timeoutMs = 180_000 } = {},
) {
  const wanted = [statuses].flat();
  const seen = [];
  let still = 0;
  return until(
    () =>
      `${swapId} to reach ${wanted.join(" | ")} (saw ${seen.join(" -> ") || "nothing"})`,
    async () => {
      const { status } = await client.swap(swapId);
      if (seen.at(-1) !== status) {
        seen.push(status);
        still = 0;
      }
      if (failOn.includes(status)) {
        throw new Error(
          `${swapId} reached ${status} (saw ${seen.join(" -> ")})`,
        );
      }
      if (wanted.includes(status)) return status;
      if (mine && ++still % 5 === 0) await mine.mine(1);
      return undefined;
    },
    { timeoutMs, everyMs: 1_500 },
  );
}

/**
 * Retries `build` (a claim or refund) while its lockup is still reaching the
 * esplora index or the maker's records.
 */
export async function settled(build, { timeoutMs = 60_000 } = {}) {
  let last;
  return until(
    () => `a spendable lockup (last error: ${last})`,
    async () => {
      try {
        return await build();
      } catch (error) {
        last = error.message;
        return undefined;
      }
    },
    { timeoutMs, everyMs: 2_000 },
  );
}

/** Mines `node` until its tip passes `height`. */
export async function mineTo(node, height) {
  const tip = await node.height();
  if (tip <= height) await node.mine(height - tip + 1);
}
