import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = new URL("../../", import.meta.url);
const contract = JSON.parse(
  readFileSync(
    process.argv[2] ?? new URL("tests/fixtures/maker-swap-schema.json", root),
    "utf8",
  ),
);
const source = ts.createSourceFile(
  "swap_types.ts",
  readFileSync(new URL("bindings-wasm/src/swap_types.ts", root), "utf8"),
  ts.ScriptTarget.Latest,
  true,
);
const aliases = new Map(
  source.statements
    .filter(ts.isTypeAliasDeclaration)
    .map((n) => [n.name.text, n]),
);

// SDK DTOs describe the value after serde, not untouched maker JSON. Maker
// nulls become undefined, and older upstream servers omit maker extensions.
const names = {
  CreateSubmarineRequest: "CreateSubmarineRequest",
  CreateSubmarineResponse: "SubmarineCreateResponse",
  CreateReverseRequest: "CreateReverseRequest",
  CreateReverseResponse: "ReverseCreateResponse",
  CreateChainRequest: "CreateChainRequest",
  CreateChainResponse: "ChainCreateResponse",
  ChainSwapDetails: "ChainSwapDetailsResponse",
  GetSwapResponse: "SwapStatus",
  SwapPayoutStatus: "PayoutStatus",
  SwapEvent: "SwapEventEntry",
  SwapRestoreResponse: "RestoredSwap",
  SwapRestoreType: "RestoredSwapType",
  ClaimDetails: "RestoreClaimDetails",
  RefundDetails: "RestoreRefundDetails",
  SwapRestoreIndexResponse: "SwapRestoreIndexResponse",
  TransactionOut: "RestoreTransaction",
  SwapTree: "SwapTree",
  Leaf: "SwapLeaf",
  RgbLock: "RgbLock",
  InternalHopRef: "InternalHopRef",
  Webhook: "WebhookConfig",
};

const ref = (name) => ({ $ref: `#/components/schemas/${name}` });
for (const [path, request, status, response] of [
  [
    "submarine",
    "CreateSubmarineRequest",
    "201",
    ref("SubmarineCreateResponse"),
  ],
  ["reverse", "CreateReverseRequest", "201", ref("ReverseCreateResponse")],
  ["chain", "CreateChainRequest", "201", ref("ChainCreateResponse")],
  ["{id}", null, "200", ref("SwapStatus")],
  [
    "restore",
    "SwapRestoreRequest",
    "200",
    { type: "array", items: ref("RestoredSwap") },
  ],
  [
    "restore/index",
    "SwapRestoreRequest",
    "200",
    ref("SwapRestoreIndexResponse"),
  ],
]) {
  assert.deepEqual(
    contract.routes[`/v2/swap/${path}`],
    {
      request: request ? ref(request) : null,
      responses: { [status]: response },
    },
    `${path}: request/response route contract changed`,
  );
}

// Deliberate unsupported maker request forms / response variants. Every
// exception names one exact field; new schema fields must be reviewed.
const omitted = {
  CreateSubmarineRequest: {
    beneficiary: "SDK requires a BOLT11 invoice and explicit refund key",
    fromAmount: "SDK uses the invoice and canonical expectedAmount",
    invoiceAmount: "BOLT12 offer create is not exposed by this SDK",
    offer: "BOLT12 offer create is not exposed by this SDK",
    pairId: "SDK selects a pair with required from/to symbols",
    preimageHash: "SDK derives the submarine payment hash from its invoice",
  },
  CreateReverseRequest: {
    beneficiary: "SDK requires an explicit claim key and payment hash/invoice",
    bolt12Refund: "Maker BOLT12 refunds are not exposed by this SDK",
    pairId: "SDK selects a pair with required from/to symbols",
  },
  CreateChainRequest: {
    fromAmount: "Legacy alias; SDK sends userLockAmount or serverLockAmount",
    pairId: "SDK selects a pair with required from/to symbols",
    userAddress: "Legacy plain-send route; SDK only builds atomic chain swaps",
  },
  CreateSubmarineResponse: {
    invoiceAmount: "Only returned for unsupported BOLT12 offer creates",
    preimageHash: "Only returned for unsupported BOLT12 offer creates",
  },
  CreateChainResponse: Object.fromEntries(
    [
      "address",
      "claimPublicKey",
      "expectedAmount",
      "payoutAmount",
      "serverLockup",
      "swapTree",
      "timeoutBlockHeight",
      "userAddress",
    ].map((field) => [
      field,
      "Legacy flat/plain-send or Arkade variant; SDK validates atomic lockupDetails/claimDetails",
    ]),
  ),
};

// Upstream Boltz fields retained by the Rust SDK, which the maker does not
// advertise. A removed supported maker field must not silently join this list.
const upstreamOnly = {
  CreateSubmarineRequest: ["referralId"],
  CreateReverseRequest: [
    "invoice",
    "description",
    "descriptionHash",
    "referralId",
  ],
  CreateChainRequest: ["referralId"],
  CreateSubmarineResponse: ["blindingKey"],
  ChainSwapDetails: ["refundAddress", "claimAddress", "bip21"],
  GetSwapResponse: ["zeroConfRejected", "transaction"],
};

// Rust DTOs retain upstream-compatible optional metadata and support narrower
// create forms than the maker's union-shaped routes. Pin each intentional
// required/optional difference so a changed required field still fails CI.
const optionalDifferences = {
  "CreateSubmarineRequest.from": "SDK selects the pair by explicit symbols",
  "CreateSubmarineRequest.to": "SDK selects the pair by explicit symbols",
  "CreateSubmarineRequest.invoice": "SDK only exposes invoice-based creates",
  "CreateReverseRequest.from": "SDK selects the pair by explicit symbols",
  "CreateReverseRequest.to": "SDK selects the pair by explicit symbols",
  "CreateReverseRequest.claimPublicKey": "SDK validates the taker's claim key",
  "CreateReverseRequest.invoiceAmount": "Upstream supports an invoice instead",
  "CreateChainRequest.from": "SDK selects the pair by explicit symbols",
  "CreateChainRequest.to": "SDK selects the pair by explicit symbols",
  "CreateChainRequest.refundPublicKey":
    "Core DTO allows upstream route variants",
  "CreateSubmarineResponse.swapAuth":
    "Older upstream makers issue no credential",
  "CreateReverseResponse.swapAuth": "Older upstream makers issue no credential",
  "CreateChainResponse.swapAuth": "Older upstream makers issue no credential",
  "CreateReverseResponse.invoice":
    "Upstream BOLT12 routes may omit the invoice",
  "CreateChainResponse.claimDetails": "SDK only exposes validated atomic swaps",
  "CreateChainResponse.lockupDetails":
    "SDK only exposes validated atomic swaps",
  "GetSwapResponse.id": "Older upstream makers return status only",
  "GetSwapResponse.type": "Older upstream makers return status only",
  "GetSwapResponse.paymentStatus": "Older upstream makers return status only",
  "GetSwapResponse.events": "Older upstream makers return status only",
  "ClaimDetails.type":
    "Older upstream restore responses omit the discriminator",
  "RefundDetails.type":
    "Older upstream restore responses omit the discriminator",
  "SwapRestoreResponse.preimageHash":
    "Older upstream restore responses omit the hash",
};

function nonNull(node) {
  if (ts.isParenthesizedTypeNode(node)) return nonNull(node.type);
  if (ts.isUnionTypeNode(node)) {
    return node.types.flatMap(nonNull);
  }
  if (
    node.kind === ts.SyntaxKind.UndefinedKeyword ||
    (ts.isLiteralTypeNode(node) &&
      node.literal.kind === ts.SyntaxKind.NullKeyword)
  )
    return [];
  return [node];
}

function normalized(schema) {
  if (schema.oneOf || schema.anyOf) {
    const variants = (schema.oneOf ?? schema.anyOf).filter(
      (s) => s.type !== "null",
    );
    assert.equal(
      variants.length,
      1,
      "new maker union needs explicit compatibility support",
    );
    return normalized(variants[0]);
  }
  return {
    ...schema,
    type: Array.isArray(schema.type)
      ? schema.type.find((v) => v !== "null")
      : schema.type,
  };
}

function compare(node, raw, context, request) {
  const schema = normalized(raw);
  const alternatives = nonNull(node);
  assert.ok(alternatives.length, `${context}: no usable SDK type`);
  if (!schema.type && !schema.$ref) {
    assert.ok(
      alternatives.every((n) => n.kind === ts.SyntaxKind.UnknownKeyword),
      `${context}: open maker JSON must remain unknown`,
    );
    return;
  }
  if (schema.$ref) {
    const ref = schema.$ref.split("/").at(-1);
    if (ref === "RestoreDetailType") {
      assert.ok(
        alternatives.every((n) => n.kind === ts.SyntaxKind.StringKeyword),
        context,
      );
      return;
    }
    assert.ok(
      alternatives.every(
        (n) =>
          ts.isTypeReferenceNode(n) &&
          names[n.typeName.getText(source)] === ref,
      ),
      `${context}: maker reference ${ref} changed`,
    );
    return;
  }
  if (schema.type === "array") {
    assert.equal(alternatives.length, 1, context);
    const n = alternatives[0];
    assert.ok(
      ts.isTypeReferenceNode(n) && n.typeName.getText(source) === "Array",
      context,
    );
    compare(n.typeArguments[0], schema.items, `${context}[]`, request);
    return;
  }
  if (schema.type === "integer") {
    const expected = request
      ? [ts.SyntaxKind.BigIntKeyword, ts.SyntaxKind.NumberKeyword]
      : schema.format === "int64"
        ? [ts.SyntaxKind.BigIntKeyword]
        : [ts.SyntaxKind.NumberKeyword];
    // Actual Rust widths determine the JS representation, despite the maker's
    // schema width: submarine timeout is SDK u64; reverse timeout is SDK u32.
    if (context === "CreateSubmarineResponse.timeoutBlockHeight")
      expected.splice(0, expected.length, ts.SyntaxKind.BigIntKeyword);
    if (context === "CreateReverseResponse.timeoutBlockHeight")
      expected.splice(0, expected.length, ts.SyntaxKind.NumberKeyword);
    assert.ok(
      alternatives.every((n) => expected.includes(n.kind)),
      `${context}: integer representation mismatch`,
    );
    return;
  }
  const expected =
    schema.type === "boolean"
      ? ts.SyntaxKind.BooleanKeyword
      : ts.SyntaxKind.StringKeyword;
  assert.ok(
    alternatives.every(
      (n) =>
        n.kind === expected ||
        (schema.type === "string" &&
          ((ts.isLiteralTypeNode(n) && ts.isStringLiteral(n.literal)) ||
            (ts.isTypeReferenceNode(n) && n.typeName.getText(source) === "T"))),
    ),
    `${context}: incompatible ${schema.type}`,
  );
}

for (const [sdk, maker] of Object.entries(names)) {
  const node = aliases.get(sdk);
  const schema = contract.schemas[maker];
  assert.ok(node && schema, `${sdk}/${maker}: missing declaration or schema`);
  if (schema.enum) {
    const values = nonNull(node.type)
      .map((n) => n.literal?.text)
      .sort();
    assert.deepEqual(values, [...schema.enum].sort(), `${sdk}: enum drift`);
    continue;
  }
  assert.ok(ts.isTypeLiteralNode(node.type), sdk);
  const fields = new Map(
    node.type.members.map((m) => [m.name.getText(source), m]),
  );
  for (const [field, shape] of Object.entries(schema.properties)) {
    if (omitted[sdk]?.[field]) {
      assert.ok(
        !fields.has(field),
        `${sdk}.${field}: stale omission exception`,
      );
      continue;
    }
    assert.ok(
      fields.has(field),
      `${sdk}.${field}: unmodeled maker field; update DTO or document the unsupported form`,
    );
    const context = `${sdk}.${field}`;
    const sdkOptional = Boolean(fields.get(field).questionToken);
    const makerOptional = !(schema.required ?? []).includes(field);
    if (optionalDifferences[context]) {
      assert.notEqual(
        sdkOptional,
        makerOptional,
        `${context}: stale optionality exception`,
      );
    } else {
      assert.equal(
        sdkOptional,
        makerOptional,
        `${context}: required/optional drift`,
      );
    }
    compare(
      fields.get(field).type,
      shape,
      `${sdk}.${field}`,
      sdk.endsWith("Request") || sdk === "Webhook",
    );
  }
  for (const field of Object.keys(omitted[sdk] ?? {})) {
    assert.ok(
      field in schema.properties,
      `${sdk}.${field}: stale maker omission exception`,
    );
  }
  for (const field of fields.keys()) {
    assert.ok(
      field in schema.properties || upstreamOnly[sdk]?.includes(field),
      `${sdk}.${field}: maker field disappeared`,
    );
  }
}
console.log(
  `Swap DTOs match the supported maker schema at ${contract.source.revision}.`,
);
console.log(
  `Schema snapshot: ${fileURLToPath(new URL("tests/fixtures/maker-swap-schema.json", root))}`,
);
