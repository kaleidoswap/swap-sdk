import {
  SwapClient,
  type CreateSubmarineRequest,
  type CreateReverseRequest,
  type CreateChainRequest,
  type SwapRestoreResponse,
} from "../../dist/index.node.js";
import type { SwapClient as BrowserClient } from "../../dist/index.js";
import type { BoltzClient as RawClient } from "../../vendor/bindings_wasm.js";

type IsAny<T> = 0 extends 1 & T ? true : false;
type AssertFalse<T extends false> = T;
type Result<M extends (...args: never[]) => unknown> = Awaited<ReturnType<M>>;
type Methods =
  | "createSubmarineSwap"
  | "createReverseSwap"
  | "createChainSwap"
  | "swap"
  | "swapRestore"
  | "swapRestoreIndex";
type NoAnyResponses<C extends Record<Methods, (...args: never[]) => unknown>> =
  {
    [M in Methods]: IsAny<Result<C[M]>>;
  }[Methods];
type PublicIsTyped = AssertFalse<NoAnyResponses<SwapClient>>;
type BrowserIsTyped = AssertFalse<NoAnyResponses<BrowserClient>>;
type WasmIsTyped = AssertFalse<NoAnyResponses<RawClient>>;
type SubmarineInputIsTyped = AssertFalse<
  IsAny<Parameters<RawClient["createSubmarineSwap"]>[1]>
>;
type ReverseInputIsTyped = AssertFalse<
  IsAny<Parameters<RawClient["createReverseSwap"]>[1]>
>;
type ChainInputIsTyped = AssertFalse<
  IsAny<Parameters<RawClient["createChainSwap"]>[1]>
>;

declare const client: SwapClient;
const submarine: CreateSubmarineRequest = {
  from: "BTC",
  to: "BTC",
  invoice: "ln...",
  refundPublicKey: "02...",
  pairHash: null,
  webhook: { url: "https://merchant.example/events", status: ["invoice.paid"] },
};
const reverse: CreateReverseRequest = {
  from: "BTC",
  to: "L-BTC",
  claimPublicKey: "02...",
  preimageHash: "ab...",
  invoiceAmount: 9007199254740993n,
};
const chain: CreateChainRequest = {
  from: "BTC",
  to: "L-BTC",
  preimageHash: "ab...",
  refundPublicKey: "02...",
  claimPublicKey: "03...",
  userLockAmount: 100_000,
};
const subResponse = await client.createSubmarineSwap("signet", submarine);
const amount: bigint = subResponse.expectedAmount;
const submarineTimeout: bigint = subResponse.timeoutBlockHeight;
const reverseResponse = await client.createReverseSwap("signet", reverse);
const reverseTimeout: number = reverseResponse.timeoutBlockHeight;
const chainResponse = await client.createChainSwap("signet", chain);
const chainAmount: bigint = chainResponse.claimDetails.amount;
const chainTimeout: number = chainResponse.claimDetails.timeoutBlockHeight;
const status = await client.swap("id");
const payoutReference: string | undefined = status.payout?.reference;
const audit: unknown = status.events?.[0]?.data;
const swaps: SwapRestoreResponse[] = await client.swapRestore("xpub", "m", 50);
for (const swap of swaps) {
  const kind: "submarine" | "reverse" | "chain" = swap.type;
  const createdAt: bigint = swap.createdAt;
  const keyIndex: number | undefined = swap.claimDetails?.keyIndex;
  const restoredAmount: bigint | undefined = swap.refundDetails?.amount;
  const paymentHash: string | undefined = swap.preimageHash;
}
const index: bigint = (await client.swapRestoreIndex("xpub", "m")).index;

// @ts-expect-error Missing required refund key.
const missingKey: CreateSubmarineRequest = {
  from: "BTC",
  to: "BTC",
  invoice: "ln...",
};
const stringAmount: CreateReverseRequest = {
  ...reverse,
  // @ts-expect-error Amounts are never strings at the WASM boundary.
  invoiceAmount: "100000",
};
// @ts-expect-error Response amounts are BigInt, not Number.
const roundedAmount: number = subResponse.expectedAmount;
// @ts-expect-error Restore type is a real discriminator, not an arbitrary string.
const wrongKind: SwapRestoreResponse["type"] = "pay-through";
// @ts-expect-error Unknown audit data must be narrowed before accessing fields.
status.events?.[0]?.data.attempt;
// @ts-expect-error Invalid payload must fail even through the raw WASM class.
client.createChainSwap("signet", { id: "only-an-id" });
