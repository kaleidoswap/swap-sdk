# Examples

These examples are intentionally small and grow in scope step by step.

## 1) Minimal TypeScript

```ts
import { SwapMasterKey, init, toJson } from "../src/index";

await init();

const key = SwapMasterKey.fromSwapMnemonic(process.env.SWAP_MNEMONIC!, "regtest");
console.log(toJson({ swapKey: key.deriveSwapKey(0n), preimage: key.derivePreimage(0n) }));
```

## 2) React Native-shaped example

```ts
import { SwapMasterKey, init, toJson } from "../src/index";

export async function runExample() {
  await init();
  const key = SwapMasterKey.fromSwapMnemonic("your swap mnemonic here", "regtest");
  console.log(toJson(key.deriveSwapKey(0n)));
}
```

## 3) Suggested next step

After these two, expand toward:

- loading the swap mnemonic from secure storage
- wiring runtime-specific WASM initialization
- creating a real quote / swap client flow
- adding a funding or claim path example
