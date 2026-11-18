import {
  type KaleidoSwap,
  type SwapPhase,
} from "@kaleidorg/swap-sdk-react-native";

/**
 * Run at every launch, before anything else. `resume()` moves each stored
 * swap one step; the ones still in flight come back in `report.resumed` and
 * are followed here to the end. A swap `resume()` could not move this pass
 * stays in the store for the next launch: it is reported, and never stops the
 * others from being followed.
 */
export async function resume(
  kaleido: KaleidoSwap,
  onPhase: (swapId: string, phase: SwapPhase) => void,
): Promise<void> {
  const report = await kaleido.resume();
  for (const e of report.errors)
    console.warn(`${e.id}: could not move this pass (${e.message})`);

  const followed = await Promise.allSettled(
    report.resumed.map((swap) =>
      swap.watch({ onPhase: (phase) => onPhase(swap.id(), phase) }),
    ),
  );

  const problems = [
    ...report.failed.map((r) => `${r.id}: failed (${r.rawStatus})`),
    ...report.errors.map((e) => `${e.id}: ${e.message}`),
    ...followed.flatMap((r) =>
      r.status === "rejected" ? [String(r.reason)] : [],
    ),
  ];
  if (problems.length) throw new Error(problems.join("\n"));
}
