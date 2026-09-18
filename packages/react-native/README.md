# @kaleidorg/swap-sdk-react-native

The KaleidoSwap swap SDK for React Native: client-side atomic swaps across
Bitcoin, Lightning, Liquid and Arkade, with keys and claim/refund construction
staying on the device.

This package is the same Rust engine the Python and browser SDKs use, compiled
for iOS and Android and exposed as a React Native turbo module. It exists
because the browser package cannot work here — Hermes has no `WebAssembly`, so
`@kaleidorg/swap-sdk`'s core will not load on a device.

## Install

```sh
npm install @kaleidorg/swap-sdk-react-native
```

Prebuilt native binaries are fetched from this version's GitHub release on
install and verified against the SHA-256 manifest carried by the npm package;
no Rust toolchain, NDK or Xcode is needed. iOS then needs a `pod install`.

With pnpm 10 or later, dependencies' lifecycle scripts do not run unless the
package is allow-listed — the install succeeds and the binaries are simply
never fetched, which surfaces as a missing native module at first import.
Allow it in `pnpm-workspace.yaml`:

```yaml
onlyBuiltDependencies:
  - "@kaleidorg/swap-sdk-react-native"
```

New architecture only, and a bare or prebuilt app — the module is native code,
so it does not run in Expo Go. With Expo, use `expo prebuild` and a development
build.

## Arkade

The Arkade Intents corridor is a separate entry point:

```ts
import { ArkadeIntentsVenue } from "@kaleidorg/swap-sdk-react-native/arkade";
```

It re-exports `@kaleidorg/swap-sdk/arkade`, which is pure TypeScript over
`@arkade-os/sdk` and runs in React Native unchanged. Install
`@kaleidorg/swap-sdk`, `@arkade-os/sdk` and `@arkade-os/swap` alongside this
package if you use it — all three are optional peers.

Metro must resolve package `exports` for the subpath to work. That is the
default from React Native 0.79; before that, set
`unstable_enablePackageExports: true` in `metro.config.js`.

## Mobile-specific notes

A swap has a deadline and a phone does not stay awake. Two consequences worth
designing around before you ship:

- **Persist the swap, and restore on launch.** `SwapMasterKey.masterXpub()`
  plus `swapRestore()` returns every swap the maker has seen for your wallet,
  including the ones still owed a claim or a refund. Call it on start rather
  than assuming in-memory state survived.
- **Persist `swapAuth`.** The maker issues it once, on the create response.
  `swapRestore` authenticates with an xpub alone and does not return it, and
  without it a chain swap that needs `acceptQuote` has no route but its refund.

The status WebSocket does not survive backgrounding, and a dead socket reports
no error. Check `isConnected()` on resume and fall back to `getSwap()` rather
than waiting on updates that will never arrive.

## Building from source

From a checkout of [kaleidoswap/swap-sdk](https://github.com/kaleidoswap/swap-sdk):

```sh
cd packages/react-native
npm install
npm run ubrn:build
```

This compiles the `bindings` crate for every target in `ubrn.config.yaml` and
regenerates the C++ and TypeScript turbo module around it. Everything it
produces is gitignored — it is reproduced from the Rust crate, not edited.

Needs the Android NDK (`ANDROID_NDK_HOME`), `cargo-ndk`, Xcode, and the Rust
targets for each platform.

## License

MIT. The swap engine is a fork of
[boltz-rust](https://github.com/SatoshiPortal/boltz-rust); see the repository
root for provenance.
