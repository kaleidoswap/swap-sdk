import { SwapMasterKey, init, toJson } from "../src/index";

await init();

const key = SwapMasterKey.fromSwapMnemonic(process.env.SWAP_MNEMONIC!, "regtest");
console.log(toJson({ swapKey: key.deriveSwapKey(0n), preimage: key.derivePreimage(0n) }));
