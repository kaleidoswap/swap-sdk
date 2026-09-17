/**
 * The Arkade Intents corridor, re-exported from `@kaleidorg/swap-sdk/arkade`.
 *
 * That venue is pure TypeScript over `@arkade-os/sdk` — no wasm — so it runs in
 * React Native unchanged, and duplicating it here would mean maintaining the
 * VHTLC logic twice. The rest of this package is the native UniFFI module,
 * because Hermes has no `WebAssembly` and the Boltz engine's browser build
 * cannot load on device.
 *
 * `@kaleidorg/swap-sdk` is an optional peer dependency: install it only if you
 * use this entry point. Metro must have package `exports` resolution enabled
 * (the default from React Native 0.79; before that, set
 * `unstable_enablePackageExports: true` in `metro.config.js`) or the subpath
 * will not resolve and the main entry — which does reference wasm — gets
 * picked up instead.
 */
export * from "@kaleidorg/swap-sdk/arkade";
