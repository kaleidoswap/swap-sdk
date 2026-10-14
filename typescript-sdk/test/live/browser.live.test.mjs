// The browser bundle against a real maker: headless Firefox loads the packed
// package and runs a whole L-BTC reverse swap — create, validate, watch over
// the WebSocket, claim, broadcast. Node pays the invoice and mines; it never
// touches the swap's keys.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer, request as httpRequest } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize, resolve } from "node:path";
import { after, before, describe, test } from "node:test";
import { fileURLToPath } from "node:url";

import { live, skip } from "./harness.mjs";

const PACKAGE_ROOT = fileURLToPath(new URL("../..", import.meta.url));
const TYPES = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

// Runs in the page. Reports progress to /log and the outcome to /result.
const PAGE = `<!doctype html>
<meta charset="utf-8">
<title>swap-sdk browser e2e</title>
<script type="module">
  const log = (message) => fetch("/log", { method: "POST", body: message });
  const post = async (path, body) => {
    const response = await fetch(path, { method: "POST", body: body ?? "" });
    if (!response.ok) throw new Error(path + ": " + (await response.text()));
    return response.text();
  };
  try {
    const config = await (await fetch("/config")).json();
    const { SwapClient, SwapMasterKey, SwapScript, SwapWsApi, init } =
      await import("/node_modules/@kaleidorg/swap-sdk/dist/index.js");
    await init();
    const maker = location.origin + "/maker/v2";
    const esplora = location.origin + "/liquid";
    const client = new SwapClient(maker);

    // Fresh keys, never leaving the page.
    const passphrase = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) =>
      b.toString(16).padStart(2, "0")).join("");
    const master = SwapMasterKey.fromWalletMnemonic(
      "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
      "regtest",
      passphrase,
    );
    const key = master.deriveSwapKey(0n);
    const preimage = master.derivePreimage(0n);

    // A maker lists Liquid pairs only while its Liquid wallet is ready.
    let card;
    for (let i = 0; i < 30 && !card; i++) {
      card = (await client.reversePairs())?.BTC?.["L-BTC"];
      if (!card) await new Promise((r) => setTimeout(r, 2000));
    }
    if (!card) throw new Error("no BTC -> L-BTC reverse pair");
    const response = await client.createReverseSwap("regtest", {
      from: "BTC",
      to: "L-BTC",
      invoiceAmount: 60000n,
      preimageHash: preimage.sha256,
      claimPublicKey: key.publicKey,
      pairHash: card.hash,
    });
    const script = SwapScript.fromReverse("liquid", "regtest", response, key.publicKey);
    await log("created " + response.id);

    const ws = new SwapWsApi(config.wsUrl);
    const loop = ws.runWsLoop();
    const updates = ws.updates();
    const seen = [];
    let reading = true;
    const reader = (async () => {
      while (reading) {
        try {
          const update = await updates.next();
          if (update?.id === response.id && seen.at(-1) !== update.status) {
            seen.push(update.status);
          }
        } catch {
          return;
        }
      }
    })();
    while (!(await ws.isConnected())) await new Promise((r) => setTimeout(r, 100));
    await ws.subscribeSwap(response.id);

    await post("/pay", response.invoice);
    const wait = async (wanted) => {
      for (let i = 0; i < 120; i++) {
        const { status } = await client.swap(response.id);
        if (wanted.includes(status)) return status;
        if (i % 4 === 3) await post("/mine");
        await new Promise((r) => setTimeout(r, 1500));
      }
      throw new Error("timed out waiting for " + wanted.join(" | "));
    };
    await wait(["transaction.confirmed"]);

    const outputAddress = await post("/address");
    let claim;
    for (let i = 0; !claim; i++) {
      try {
        claim = await script.constructClaim(preimage.preimage, {
          outputAddress,
          swapId: response.id,
          keysSecretHex: key.secretKey,
          makerBaseUrl: maker,
          network: "regtest",
          liquidEsploraUrl: esplora,
          feeSatPerVb: 1,
          cooperative: false,
        });
      } catch (error) {
        if (i > 20) throw error;
        await new Promise((r) => setTimeout(r, 1500));
      }
    }
    const txid = await claim.broadcast("regtest", undefined, esplora);
    await log("claim " + txid);
    await post("/mine");
    await wait(["invoice.settled"]);
    for (let i = 0; i < 40 && !seen.includes("invoice.settled"); i++) {
      await new Promise((r) => setTimeout(r, 250));
    }
    reading = false;
    await ws.close();
    await loop;
    void reader;
    await post("/result", JSON.stringify({ ok: true, id: response.id, txid, seen }));
  } catch (error) {
    await post("/result", JSON.stringify({ ok: false, error: error?.stack ?? String(error) }));
  }
</script>
`;

/** Forwards `request` to `target` + the path after `prefix`. */
function proxy(request, response, target, prefix) {
  const url = new URL(target);
  const upstream = httpRequest(
    {
      host: url.hostname,
      port: url.port,
      method: request.method,
      path: url.pathname.replace(/\/$/, "") + request.url.slice(prefix.length),
      headers: { ...request.headers, host: url.host },
    },
    (answer) => {
      response.writeHead(answer.statusCode, answer.headers);
      answer.pipe(response);
    },
  );
  upstream.on("error", (error) => response.writeHead(502).end(error.message));
  request.pipe(upstream);
}

const body = (request) =>
  new Promise((resolveBody) => {
    let text = "";
    request.on("data", (chunk) => (text += chunk));
    request.on("end", () => resolveBody(text));
  });

describe("browser bundle", { skip }, () => {
  let env;
  let root;
  let tarball;

  before(async () => {
    env = await live();
    root = mkdtempSync(join(tmpdir(), "swap-sdk-browser-e2e-"));
    const packed = JSON.parse(
      execFileSync(
        "npm",
        ["pack", "--json", "--ignore-scripts", "--pack-destination", root],
        { cwd: PACKAGE_ROOT, encoding: "utf8" },
      ),
    );
    // npm 10 answers with an array, npm 12 with an object keyed by name.
    const { filename } = Array.isArray(packed)
      ? packed[0]
      : Object.values(packed)[0];
    tarball = join(root, filename);
    writeFileSync(
      join(root, "package.json"),
      '{"private":true,"type":"module"}',
    );
    execFileSync(
      "npm",
      ["install", "--ignore-scripts", "--no-audit", "--no-fund", tarball],
      { cwd: root, stdio: "ignore" },
    );
    writeFileSync(join(root, "index.html"), PAGE);
  });

  after(() => {
    if (root) rmSync(root, { recursive: true, force: true });
  });

  test("Firefox runs a reverse swap from the packed bundle", async () => {
    let finish;
    const outcome = new Promise((resolveOutcome) => (finish = resolveOutcome));
    const server = createServer(async (request, response) => {
      const path = new URL(request.url, "http://x").pathname;
      try {
        if (path.startsWith("/maker/"))
          return proxy(
            request,
            response,
            env.makerUrl.replace(/\/v2\/?$/, ""),
            "/maker",
          );
        if (path.startsWith("/liquid/"))
          return proxy(request, response, env.liquidEsplora, "/liquid");
        if (path === "/config") {
          return response.end(JSON.stringify({ wsUrl: env.wsUrl }));
        }
        if (path === "/log") {
          console.log(`  [browser] ${await body(request)}`);
          return response.writeHead(204).end();
        }
        if (path === "/pay") {
          await env.ln.pay(await body(request));
          return response.writeHead(204).end();
        }
        if (path === "/mine") {
          await env.liquid.mine(1);
          return response.writeHead(204).end();
        }
        if (path === "/address") {
          return response.end(await env.liquid.newAddress());
        }
        if (path === "/result") {
          finish(JSON.parse(await body(request)));
          return response.writeHead(204).end();
        }
        const file = resolve(
          root,
          normalize(path === "/" ? "index.html" : path.slice(1)),
        );
        if (!file.startsWith(`${root}/`)) return response.writeHead(403).end();
        const data = readFileSync(file);
        response.writeHead(200, {
          "content-type":
            TYPES.get(extname(file)) ?? "application/octet-stream",
        });
        return response.end(data);
      } catch (error) {
        response
          .writeHead(error.code === "ENOENT" ? 404 : 500)
          .end(String(error));
      }
    });
    await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
    const profile = join(root, "profile");
    mkdirSync(profile);
    const browser = spawn(
      process.env.BROWSER_BIN ?? "firefox",
      [
        "--headless",
        "--no-remote",
        "--profile",
        profile,
        `http://127.0.0.1:${server.address().port}/`,
      ],
      { stdio: "ignore" },
    );
    browser.on("exit", (code, signal) =>
      finish({ ok: false, error: `browser exited (${code ?? signal})` }),
    );
    const timer = setTimeout(
      () =>
        finish({ ok: false, error: "the page reported nothing in 5 minutes" }),
      300_000,
    );
    try {
      const result = await outcome;
      assert.ok(result.ok, result.error);
      assert.ok(await env.liquid.knows(result.txid), "the claim is on chain");
      assert.ok(
        result.seen.includes("invoice.settled"),
        `the page's WebSocket saw ${result.seen.join(" -> ")}`,
      );
    } finally {
      clearTimeout(timer);
      browser.kill("SIGTERM");
      server.closeAllConnections();
      await new Promise((closed) => server.close(closed));
    }
  });
});
