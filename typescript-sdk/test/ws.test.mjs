import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import test from "node:test";
import { init, SwapWsApi } from "../dist/index.node.js";

await init();

/** Resolves when `promise` settles, or rejects after `ms`. */
function within(ms, promise, what) {
  return Promise.race([
    promise,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`${what} after ${ms} ms`)), ms),
    ),
  ]);
}

test("close() ends runWsLoop while it waits to reconnect", async () => {
  // Nothing listens on port 1.
  const ws = new SwapWsApi("ws://127.0.0.1:1/v2/ws");
  const loop = ws.runWsLoop();
  await new Promise((resolve) => setTimeout(resolve, 200));
  await ws.close();
  await within(5_000, loop, "runWsLoop still running");
  assert.equal(await ws.isConnected(), false);
});

test("close() sends a close frame on a connected socket and ends the loop", async () => {
  let opcode;
  let upgraded;
  const server = createServer();
  const frame = new Promise((resolve) => {
    server.on("upgrade", (req, socket) => {
      upgraded = socket;
      const accept = createHash("sha1")
        .update(
          req.headers["sec-websocket-key"] +
            "258EAFA5-E914-47DA-95CA-C5AB0DC85B11",
        )
        .digest("base64");
      socket.write(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n" +
          `Connection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`,
      );
      socket.once("data", (data) => resolve(data[0] & 0x0f));
      socket.on("error", () => undefined);
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const ws = new SwapWsApi(`ws://127.0.0.1:${server.address().port}/v2/ws`);
    const loop = ws.runWsLoop();
    await within(
      5_000,
      (async () => {
        while (!(await ws.isConnected())) {
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
      })(),
      "never connected",
    );
    await ws.close();
    await within(5_000, loop, "runWsLoop still running");
    opcode = await within(5_000, frame, "no frame");
    assert.equal(opcode, 0x8);
  } finally {
    upgraded?.destroy();
    server.close();
  }
});
