import assert from "node:assert/strict";
import { readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import {
  ARCHIVES,
  download,
  allowsMissingNativeLibraries,
  unreachableArchiveError,
  unreachableArchiveWarning,
  validateManifest,
} from "../scripts/native-artifacts.mjs";

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

test("an install keeps the native libraries unless it asks not to", () => {
  assert.equal(allowsMissingNativeLibraries({}), false);
  for (const value of ["", "0", "false", "no", " FALSE "]) {
    assert.equal(
      allowsMissingNativeLibraries({ KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE: value }),
      false,
      value,
    );
  }
  for (const value of ["1", "true", "yes", "anything"]) {
    assert.equal(
      allowsMissingNativeLibraries({ KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE: value }),
      true,
      value,
    );
  }
});

const unreachable = {
  archive: ARCHIVES[0],
  url: `https://example.invalid/v0.7.2/${ARCHIVES[0]}`,
  version: "0.7.2",
  reason: "download returned HTTP 404",
};

test("the install failure carries every way forward", () => {
  const message = unreachableArchiveError(unreachable);
  // This is the text npm prints, and the only chance to explain both that the
  // archives live outside the registry and that there is a way past it.
  for (const fragment of [
    ARCHIVES[0],
    "0.7.2",
    "HTTP 404",
    "example.invalid",
    "ubrn:build",
    "KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE=1",
  ]) {
    assert.ok(message.includes(fragment), `failure omits ${fragment}`);
  }
});

test("the opted-in warning says what was installed and why", () => {
  const warning = unreachableArchiveWarning(unreachable);
  for (const fragment of [
    "WARNING: installed WITHOUT its native libraries",
    "KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE",
    ARCHIVES[0],
    "HTTP 404",
  ]) {
    assert.ok(warning.includes(fragment), `warning omits ${fragment}`);
  }
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
    new Promise((resolve, reject) => {
      calls += 1;
      if (calls > 1) return resolve(new Response("archive", { status: 200 }));
      // A real fetch holds a socket open while it waits; this fake holds a
      // timer instead. Without one, the deadline's own timer is unref'd and
      // the event loop drains before it fires — on Node 22 the runner then
      // cancels the test as a promise that can never settle.
      const socket = setTimeout(() => {}, 10_000);
      signal.addEventListener("abort", () => {
        clearTimeout(socket);
        reject(signal.reason);
      });
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
