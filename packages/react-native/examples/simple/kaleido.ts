import {
  FileStore,
  KaleidoSwap,
  KaleidoSwapOptions,
  type ResumeReport,
  MnemonicSigner,
  Network,
} from "@kaleidorg/swap-sdk-react-native";

export async function openKaleidoSwap(
  storeDir: string,
  swapMnemonic: string,
): Promise<{ kaleido: KaleidoSwap; report: ResumeReport }> {
  const kaleido = new KaleidoSwap(
    KaleidoSwapOptions.create({
      network: Network.Signet,
      store: new FileStore(storeDir),
      signer: new MnemonicSigner(swapMnemonic, undefined, Network.Signet),
    }),
  );
  const report = await kaleido.resume();
  if (report.errors.length)
    throw new Error(`resume could not move ${report.errors.map((e) => e.id)}`);
  return { kaleido, report };
}
