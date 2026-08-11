import { spawn, execFileSync } from "node:child_process";
import {
  createReadStream,
  mkdtempSync,
  mkdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, isAbsolute, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// The tarball argument is optional, as in smoke-package.mjs: a release passes the
// exact archive it is about to publish, and a pull request packs a throwaway one
// so `npm run smoke:browser-package` needs no setup.
const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const suppliedTarball = process.argv[2];
let tarballPath = suppliedTarball
  ? isAbsolute(suppliedTarball)
    ? suppliedTarball
    : resolve(process.cwd(), suppliedTarball)
  : undefined;
let removeTarball = false;
const consumerRoot = mkdtempSync(join(tmpdir(), "kaleidorg-swap-sdk-browser-"));
const profileRoot = join(consumerRoot, "firefox-profile");
const browserBin = process.env.BROWSER_BIN ?? "firefox";
mkdirSync(profileRoot);

const contentTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

try {
  if (!tarballPath) {
    const [{ filename }] = JSON.parse(
      execFileSync("npm", ["pack", "--json"], {
        cwd: packageRoot,
        encoding: "utf8",
      }),
    );
    tarballPath = join(packageRoot, filename);
    removeTarball = true;
  }

  writeFileSync(
    join(consumerRoot, "package.json"),
    JSON.stringify({ private: true, type: "module" }),
  );
  execFileSync(
    "npm",
    ["install", "--ignore-scripts", "--no-audit", "--no-fund", tarballPath],
    {
      cwd: consumerRoot,
      stdio: "inherit",
    },
  );

  writeFileSync(
    join(consumerRoot, "index.html"),
    `<!doctype html>
      <meta charset="utf-8">
      <title>KaleidoSwap SDK package smoke test</title>
      <script type="module">
        try {
          const { SwapMasterKey, init, toJson } = await import(
            "/node_modules/@kaleidorg/swap-sdk/dist/index.js"
          );
          await init();
          const key = SwapMasterKey.fromWalletMnemonic(
            "slogan prevent affair connect autumn crop together earn track ribbon horn copy",
            "regtest",
          );
          if (key.masterXpub().length === 0) {
            throw new Error("native key derivation returned an empty xpub");
          }
          if (toJson({ amount: 1n }) !== '{"amount":"1"}') {
            throw new Error("bigint JSON encoding failed");
          }
          await fetch("/result?status=pass");
        } catch (error) {
          await fetch(
            "/result?status=fail&message=" +
              encodeURIComponent(error?.stack ?? String(error)),
          );
        }
      </script>
    `,
  );

  let finish;
  const result = new Promise((resolveResult) => {
    finish = resolveResult;
  });
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    if (url.pathname === "/result") {
      const status = url.searchParams.get("status");
      const message = url.searchParams.get("message");
      response.writeHead(204).end();
      finish({ status, message });
      return;
    }

    const requestPath = url.pathname === "/" ? "/index.html" : url.pathname;
    const relativePath = normalize(decodeURIComponent(requestPath)).replace(
      /^[/\\]+/,
      "",
    );
    const filePath = resolve(consumerRoot, relativePath);
    if (filePath !== consumerRoot && !filePath.startsWith(`${consumerRoot}/`)) {
      response.writeHead(403).end("forbidden");
      return;
    }
    try {
      if (!statSync(filePath).isFile()) {
        throw new Error("not a file");
      }
    } catch {
      response.writeHead(404).end("not found");
      return;
    }
    const stream = createReadStream(filePath);
    response.writeHead(200, {
      "Content-Type":
        contentTypes.get(extname(filePath)) ?? "application/octet-stream",
    });
    stream.pipe(response);
  });

  await new Promise((resolveListen, rejectListen) => {
    const reject = (error) => rejectListen(error);
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
      resolveListen();
    });
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("browser smoke server did not expose a TCP port");
  }

  const browser = spawn(
    browserBin,
    [
      "--headless",
      "--no-remote",
      "--profile",
      profileRoot,
      `http://127.0.0.1:${address.port}/`,
    ],
    { stdio: "inherit" },
  );
  browser.on("error", (error) =>
    finish({ status: "fail", message: error.message }),
  );
  browser.on("exit", (code, signal) =>
    finish({
      status: "fail",
      message: `browser exited before reporting success (${code ?? signal})`,
    }),
  );

  const timeout = setTimeout(
    () => finish({ status: "fail", message: "browser smoke test timed out" }),
    60_000,
  );
  const outcome = await result;
  clearTimeout(timeout);
  browser.kill("SIGTERM");
  await new Promise((resolveClose) => server.close(resolveClose));

  if (outcome.status !== "pass") {
    throw new Error(`browser package smoke test failed: ${outcome.message}`);
  }
  console.log(`browser package smoke test passed: ${tarballPath}`);
} finally {
  if (removeTarball && tarballPath) {
    rmSync(tarballPath, { force: true });
  }
  rmSync(consumerRoot, { recursive: true, force: true });
}
