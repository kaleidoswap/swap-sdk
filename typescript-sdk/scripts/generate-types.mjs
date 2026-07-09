// Generates src/generated/node-types.ts from the RLN OpenAPI 3.1 spec.
//
// Uses the openapi-typescript Node API (instead of the CLI) so we can apply a
// transform: every `type: integer` schema is emitted as `bigint`, matching the
// wasm boundary, which serializes Rust i64/u64 as JS BigInt
// (serialize_large_number_types_as_bigints) to avoid f64 precision loss on
// u64 amounts. One rule, uniform: integers from RLN are `bigint`.
//
// Run with: npm run generate:types   (or: make generate-ts-types)
import fs from "node:fs";
import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const SPEC = new URL("../../specs/rgb-lightning-node.yaml", import.meta.url);
const OUT = new URL("../src/generated/node-types.ts", import.meta.url);

const BIGINT = ts.factory.createKeywordTypeNode(ts.SyntaxKind.BigIntKeyword);
const NULL = ts.factory.createLiteralTypeNode(ts.factory.createNull());

const ast = await openapiTS(SPEC, {
  exportType: true,
  enum: true,
  dedupeEnums: true,
  transform(schemaObject) {
    // Plain integer → bigint; OpenAPI 3.1 nullable integer → bigint | null.
    if (schemaObject.type === "integer") {
      return BIGINT;
    }
    if (
      Array.isArray(schemaObject.type) &&
      schemaObject.type.includes("integer") &&
      schemaObject.type.includes("null") &&
      schemaObject.type.length === 2
    ) {
      return ts.factory.createUnionTypeNode([BIGINT, NULL]);
    }
    return undefined;
  },
});

const banner = `/**
 * AUTO-GENERATED FILE — DO NOT EDIT MANUALLY.
 *
 * Re-generate with:
 *   npm run generate:types
 *   (or: make generate-ts-types)
 *
 * NOTE: integer fields are typed \`bigint\` to match the wasm boundary's
 * lossless BigInt serialization of Rust i64/u64.
 */
`;

fs.writeFileSync(OUT, banner + astToString(ast));
console.log(`✔ wrote ${OUT.pathname}`);
