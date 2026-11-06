import {
  Amount,
  Asset,
  type KaleidoSwap,
  type SwapPhase,
} from "@kaleidorg/swap-sdk-react-native";

export async function submarine(
  kaleido: KaleidoSwap,
  bolt11Invoice: string,
  refundAddress: string,
  onFunding: (address: string, sats: bigint) => void,
  onPhase: (phase: SwapPhase) => void,
): Promise<void> {
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
