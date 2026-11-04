import { type SwapPhase } from "@kaleidorg/swap-sdk-react-native";

import { openKaleidoSwap } from "./kaleido";

export async function resume(
  storeDir: string,
  swapMnemonic: string,
  onPhase: (swapId: string, phase: SwapPhase) => void,
): Promise<void> {
  const { report } = await openKaleidoSwap(storeDir, swapMnemonic);

  const followed = await Promise.allSettled(
    report.resumed.map((swap) =>
      swap.watch({ onPhase: (phase) => onPhase(swap.id(), phase) }),
    ),
  );

  const problems = [
    ...report.failed.map((r) => `${r.id}: failed (${r.rawStatus})`),
    ...followed.flatMap((r) =>
      r.status === "rejected" ? [String(r.reason)] : [],
    ),
  ];
  if (problems.length) throw new Error(problems.join("\n"));
}
