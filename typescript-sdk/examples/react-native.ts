import { SwapMasterKey, init, toJson } from "../src/index";

export async function runExample() {
  await init();
  const key = SwapMasterKey.fromSwapMnemonic("your swap mnemonic here", "regtest");
  console.log(toJson(key.deriveSwapKey(0n)));
}
