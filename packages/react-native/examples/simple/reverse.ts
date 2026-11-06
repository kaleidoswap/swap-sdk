import {
  Amount,
  Asset,
  type SwapPhase,
} from "@kaleidorg/swap-sdk-react-native";

import { openKaleidoSwap } from "./kaleido";

export async function reverse(
  storeDir: string,
  swapMnemonic: string,
  liquidAddress: string,
  onInvoice: (invoice: string) => void,
  onPhase: (phase: SwapPhase) => void,
): Promise<void> {
  const { kaleido } = await openKaleidoSwap(storeDir, swapMnemonic);

  const { quote, refusal } = await kaleido.quote({
    pay: new Asset.BtcLightning(),
    receive: new Asset.BtcLiquid(),
    amount: new Amount.Pay(50_000n),
  });
  if (!quote) throw new Error(refusal!.message);

  const swap = await kaleido.start(quote, {
    destination: liquidAddress,
    refundAddress: undefined,
  });
  onInvoice(swap.action().invoice!);
  await swap.watch({ onPhase });
}
