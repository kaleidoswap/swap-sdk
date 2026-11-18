import {
  FileStore,
  KaleidoSwap,
  KaleidoSwapOptions,
  MnemonicSigner,
  Network,
} from "@kaleidorg/swap-sdk-react-native";

/**
 * Builds the one `KaleidoSwap` the app uses. Call it once, at app scope, and
 * pass the instance to `resume`, `submarine` and `reverse`: one `FileStore`
 * and one `KaleidoSwap` per store directory per process.
 *
 * `storeDir` belongs to this swap mnemonic alone. Keep it in app-private
 * storage that is not backed up, and never reset, copy or restore it from an
 * older backup while the mnemonic is in use. If it is lost, start a new swap
 * mnemonic.
 */
export function openKaleidoSwap(
  storeDir: string,
  swapMnemonic: string,
): KaleidoSwap {
  return new KaleidoSwap(
    KaleidoSwapOptions.create({
      network: Network.Signet,
      store: new FileStore(storeDir),
      signer: new MnemonicSigner(swapMnemonic, undefined, Network.Signet),
    }),
  );
}
