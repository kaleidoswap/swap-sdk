import {
  Amount,
  Asset,
  type SwapPhase,
} from "@kaleidorg/swap-sdk-react-native";

import { openKaleidoSwap } from "./kaleido";

export async function submarine(
  storeDir: string,
  swapMnemonic: string,
  bolt11Invoice: string,
  refundAddress: string,
  onFunding: (address: string, sats: bigint) => void,
  onPhase: (phase: SwapPhase) => void,
): Promise<void> {
  const { kaleido } = await openKaleidoSwap(storeDir, swapMnemonic);

  const { quote, refusal } = await kaleido.quote({
    pay: new Asset.BtcOnchain(),
    receive: new Asset.BtcLightning(),
    amount: new Amount.Invoice(bolt11Invoice),
  });
  if (!quote) throw new Error(refusal!.message);

  const swap = await kaleido.start(quote, {
    destination: undefined,
    refundAddress,
  });
  const { fundingAddress, fundingAmount } = swap.action();
  onFunding(fundingAddress!, fundingAmount!);
  await swap.watch({ onPhase });
}
