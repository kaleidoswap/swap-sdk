import assert from "node:assert/strict";
import { test } from "node:test";

import { reorderModifiers } from "../scripts/fix-generated-modifiers.mjs";

test("reorders an async method's modifiers", () => {
  assert.equal(
    reorderModifiers("    async public getHeight(): Promise<HeightResponse> {"),
    "    public async getHeight(): Promise<HeightResponse> {",
  );
});

test("reorders an async static constructor's modifiers", () => {
  assert.equal(
    reorderModifiers("    async public static fromMnemonic(m: string) {"),
    "    public static async fromMnemonic(m: string) {",
  );
});

test("leaves correctly ordered modifiers alone", () => {
  const good = "    public async getHeight(): Promise<HeightResponse> {";
  assert.equal(reorderModifiers(good), good);
});

test("leaves top-level async functions alone", () => {
  // These render correctly: the declaration keyword goes in the slot that the
  // object path misuses for visibility.
  const fn = "export async function swapRestore(xpub: string) {";
  assert.equal(reorderModifiers(fn), fn);
});

test("does not touch an identifier that merely ends in `async`", () => {
  const line = "  const isAsync = publicAsyncThing;";
  assert.equal(reorderModifiers(line), line);
});

test("rewrites every occurrence, not just the first", () => {
  const source = [
    "  async public a(): Promise<void> {}",
    "  async public b(): Promise<void> {}",
  ].join("\n");
  assert.equal(
    reorderModifiers(source),
    ["  public async a(): Promise<void> {}", "  public async b(): Promise<void> {}"].join(
      "\n",
    ),
  );
});
