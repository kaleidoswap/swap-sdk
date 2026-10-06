/**
 * RGB wallet adapter example. This exports integration helpers, not a live swap
 * CLI. The wallet implements rgb-lib calls; the SDK never imports rgb-lib.
 */
import {
  type ColoredRgbPsbt,
  type FinalizedRgbSpend,
  type RgbPsbtParams,
  type RgbPsbtTemplate,
  type SwapScript,
} from "@kaleidorg/swap-sdk";

export interface RgbWalletAdapter {
  /** Add spendable BTC fee inputs/change to an uncolored PSBT. Sign nothing. */
  fundBitcoinPsbt(template: RgbPsbtTemplate): Promise<string>;
  /**
   * Call rgb-lib's psbt_op_prepare_with_expiry using template.swapOutpoint,
   * assetId and output_map { [paymentOutputIndex]: amount }. Return its actual
   * allocations, not metadata copied from the template. Keep the operation ID.
   */
  colorPsbt(template: RgbPsbtTemplate): Promise<{
    coloredPsbt: ColoredRgbPsbt;
    operationId: string;
  }>;
}

export interface RgbSpendResult extends FinalizedRgbSpend {
  operationId: string;
}

async function finalizeRgbSpend(
  script: SwapScript,
  params: RgbPsbtParams,
  wallet: RgbWalletAdapter,
  keysSecretHex: string,
  preimageHex?: string,
): Promise<RgbSpendResult> {
  let spend =
    preimageHex === undefined
      ? await script.prepareRgbRefund(params)
      : await script.prepareRgbClaim(params);
  try {
    if (spend.template().requiresFunding) {
      const fundedPsbt = await wallet.fundBitcoinPsbt(spend.template());
      const funded = spend.fund(fundedPsbt);
      spend.free();
      spend = funded;
    }
    const { coloredPsbt, operationId } = await wallet.colorPsbt(
      spend.template(),
    );
    const finalized =
      preimageHex === undefined
        ? spend.finalizeRefund(coloredPsbt, keysSecretHex)
        : spend.finalizeClaim(coloredPsbt, keysSecretHex, preimageHex);
    return { ...finalized, operationId };
  } finally {
    spend.free();
  }
}

/** Call after rgb-lib has accepted the maker's lock consignment and confirmations. */
export function finalizeRgbClaim(
  script: SwapScript,
  params: RgbPsbtParams,
  wallet: RgbWalletAdapter,
  claimSecretHex: string,
  preimageHex: string,
): Promise<RgbSpendResult> {
  return finalizeRgbSpend(script, params, wallet, claimSecretHex, preimageHex);
}

/** Use CallerInputs when the submarine lock's sats cannot fund a refund. */
export function finalizeRgbRefund(
  script: SwapScript,
  params: RgbPsbtParams,
  wallet: RgbWalletAdapter,
  refundSecretHex: string,
): Promise<RgbSpendResult> {
  return finalizeRgbSpend(script, params, wallet, refundSecretHex);
}

// After these helpers: preserve the SDK's HTLC witness while signing/finalizing
// wallet fee inputs. Persist the operation and spend before broadcasting. Use
// psbt_op_mark_broadcast, broadcast the final tx, psbt_op_apply, and
// psbt_op_provide_receive_consignment for the destination receive. Wait for the
// refund timeout before broadcasting a refund, and confirm a claim before it.
// Free result.transaction, when present, after use.
