import {
  canClaim,
  canRefund,
  refundSubmarineSwap,
  refundChainSwap,
  claimReverseSwap,
  claimChainSwap,
  type RefundSwapOptions,
  type ClaimSwapOptions,
  type ClaimChainOptions,
  type SwapSpendResult,
  type SignedSwapSpend,
} from "../dist/index.node.js";
import {
  claimChainSwap as browserClaim,
  type SpendEligibility,
} from "../dist/index.js";

declare const refund: RefundSwapOptions;
declare const reverse: ClaimSwapOptions;
declare const chain: ClaimChainOptions;
const results: Promise<SwapSpendResult>[] = [
  refundSubmarineSwap(refund),
  refundChainSwap(refund),
  claimReverseSwap(reverse),
  claimChainSwap(chain),
  browserClaim(chain),
];
const eligibility: SpendEligibility[] = [
  canClaim("transaction.confirmed", 1n, { timeoutBlockHeight: 2 }),
  canRefund("swap.expired", 1, { timeoutBlockHeight: 2n }),
];
async function retry(): Promise<void> {
  const first = await refundSubmarineSwap(refund);
  if (first.kind === "signed") {
    const saved: SignedSwapSpend = first;
    const hex: string = first.transaction.hex();
    await refundSubmarineSwap({ ...refund, previous: saved });
    void hex;
  } else {
    const reason: string = first.reason;
    // @ts-expect-error No transaction exists when construction is not eligible.
    first.transaction.hex();
    void reason;
  }
}
// @ts-expect-error Claims require a preimage, refunds do not.
claimReverseSwap(refund);
refundSubmarineSwap({
  ...refund,
  // @ts-expect-error Fee units remain numerical satoshis/vbyte.
  params: { ...refund.params, feeSatPerVb: "1" },
});
// @ts-expect-error Chain context must be an actual script.
claimChainSwap({ ...chain, lockupScript: "script" });
// @ts-expect-error Snapshot height is an exact integer or bigint, not a string.
canClaim("transaction.confirmed", "1", { timeoutBlockHeight: 2 });
void results;
void eligibility;
void retry;
