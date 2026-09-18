import assert from "node:assert/strict";
import { test } from "node:test";

import { ARCHIVES, validateManifest } from "../scripts/native-artifacts.mjs";

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
