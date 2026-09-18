import assert from "node:assert/strict";
import { readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { ARCHIVES, download, validateManifest } from "../scripts/native-artifacts.mjs";

const digest = "a".repeat(64);
const valid = {
  schema: 1,
  version: "0.7.2",
  artifacts: Object.fromEntries(ARCHIVES.map((name) => [name, { sha256: digest }])),
};

test("accepts the exact two-archive integrity manifest", () => {
  assert.doesNotThrow(() => validateManifest(valid, "0.7.2"));
});

test("rejects a manifest for a different package version", () => {
  assert.throws(() => validateManifest(valid, "0.7.3"), /identity mismatch/);
});

test("rejects a missing or malformed digest", () => {
  const malformed = structuredClone(valid);
  malformed.artifacts[ARCHIVES[0]].sha256 = "not-a-sha256";
  assert.throws(() => validateManifest(malformed, "0.7.2"), /no valid SHA-256/);
});

test("rejects unlisted release files", () => {
  const extra = structuredClone(valid);
  extra.artifacts["surprise.zip"] = { sha256: digest };
  assert.throws(() => validateManifest(extra, "0.7.2"), /unexpected files/);
});

// A fetch that fails `failures` times before answering, so the retry policy can
// be exercised without a network.
function flakyFetch(failures, { status = 503, body = "archive" } = {}) {
  let calls = 0;
  const fetch = async (_url, { signal }) => {
    calls += 1;
    assert.ok(signal instanceof AbortSignal, "every attempt carries a deadline");
    if (calls <= failures) return new Response(null, { status });
    return new Response(body, { status: 200 });
  };
  return { fetch, calls: () => calls };
}

const fast = { attempts: 3, backoffMs: 0, timeoutMs: 1_000 };

test("download retries a transient server error and then succeeds", async () => {
  const destination = join(tmpdir(), `download-retry-${process.pid}`);
  const remote = flakyFetch(2);
  await download("https://example.invalid/a.zip", destination, { ...remote, ...fast });
  assert.equal(remote.calls(), 3);
  assert.equal(await readFile(destination, "utf8"), "archive");
  await rm(destination, { force: true });
});

test("download does not retry a missing release asset", async () => {
  const destination = join(tmpdir(), `download-404-${process.pid}`);
  const remote = flakyFetch(5, { status: 404 });
  await assert.rejects(
    download("https://example.invalid/a.zip", destination, { ...remote, ...fast }),
    /HTTP 404/,
  );
  assert.equal(remote.calls(), 1);
  await assert.rejects(stat(destination), { code: "ENOENT" });
});

test("download gives up after its attempts and leaves nothing behind", async () => {
  const destination = join(tmpdir(), `download-exhausted-${process.pid}`);
  const remote = flakyFetch(5);
  await assert.rejects(
    download("https://example.invalid/a.zip", destination, { ...remote, ...fast }),
    /HTTP 503/,
  );
  assert.equal(remote.calls(), 3);
  await assert.rejects(stat(destination), { code: "ENOENT" });
});

test("download retries a connection that never answers", async () => {
  const destination = join(tmpdir(), `download-timeout-${process.pid}`);
  let calls = 0;
  const fetch = (_url, { signal }) =>
    new Promise((_resolve, reject) => {
      calls += 1;
      if (calls > 1) return _resolve(new Response("archive", { status: 200 }));
      signal.addEventListener("abort", () => reject(signal.reason));
    });
  await download("https://example.invalid/a.zip", destination, {
    fetch,
    attempts: 2,
    backoffMs: 0,
    timeoutMs: 50,
  });
  assert.equal(calls, 2);
  assert.equal(await readFile(destination, "utf8"), "archive");
  await rm(destination, { force: true });
});
