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

If the release cannot be reached — a network that allows the npm registry but
not `github.com`, or a release whose assets are gone — the install fails with a
message naming the archive, the URL and the reason. Reinstall once the release
is reachable, or build from source with `npm run ubrn:build` in a checkout.

An install that has its own reason to continue without the binaries — an
offline mirror that compiles from source, a CI stage that never links the app —
sets `KALEIDO_SWAP_SDK_ALLOW_MISSING_NATIVE=1`. The install then warns and
succeeds, leaving the same state as the pnpm case below: a missing native module
at build or first import. Note that npm hides a dependency's install output
unless you pass `--foreground-scripts`, so that warning is easy to miss — which
is why it is opt-in. An archive that arrives and does not match the SHA-256
manifest is always a hard failure, whatever this variable says.

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

## Partner attribution: keep the key off the device

`SwapClient.kaleidoMaker(makerUrl, apiKey, timeout)` builds a client that
attributes its swaps to a partner organization with a `kld_test_…` /
`kld_live_…` key. That key is a permanent organization credential with no
origin binding and no per-key rate limit. Anything inside an app bundle is
readable by everyone who installs the app, who can then attribute their own
swaps to your organization or exhaust its limits.

So in this package `kaleidoMaker` **throws** `Exception.Generic` before the key
reaches native code, unless you pass an explicit opt-in as the fourth argument:

```ts
import { SwapClient } from "@kaleidorg/swap-sdk-react-native";

// Throws: refusing to build an attributed client in a mobile app.
SwapClient.kaleidoMaker(makerUrl, apiKey, undefined);

// Builds. Only do this if you have accepted that every user holds the key.
SwapClient.kaleidoMaker(makerUrl, apiKey, undefined, { allowMobile: true });
```

Only the boolean `true` counts. For attribution in a mobile app, use
`SwapClient.kaleidoOrg` with the organization's registered public key. Public
organization keys are safe to ship in the app; secret maker API keys belong in
server-side configuration. The browser SDK offers `createKaleidoOrgClient` for
the same purpose. Its secret-key factory, `forKaleidoMaker`, requires an explicit
`allowBrowser: true` override in a page, worker or extension.

## High-level API: `KaleidoSwap`

For a swap rather than a protocol. `quote` prices a route, `start` creates the
swap and returns what to pay or fund, and `watch` follows the swap to the end,
claiming or refunding on its own; `resume` finishes or refunds whatever a
previous run left in the store. The store and the signer are yours: the SDK
holds no key material and never funds anything on your behalf.
[`examples/`](examples/) has a submarine swap, a reverse swap and a resume at
launch.

```ts
import {
  FileStore,
  KaleidoSwap,
  KaleidoSwapOptions,
  MnemonicSigner,
  Network,
} from "@kaleidorg/swap-sdk-react-native";

// Once, at app scope: one FileStore and one KaleidoSwap per store directory.
const kaleido = new KaleidoSwap(
  KaleidoSwapOptions.create({
    network: Network.Signet,
    store: new FileStore(storeDir),
    signer: new MnemonicSigner(swapMnemonic, undefined, Network.Signet),
  }),
);
const report = await kaleido.resume();
```

`FileStore` keeps one JSON file per swap, owner-readable only. `MemoryStore`
keeps nothing past the process: use it in tests only, never for a swap you
fund. Records carry the maker's `swapAuth` and the material a refund needs, so
keep the swap mnemonic in the Keychain or Keystore and put `storeDir` in
app-private storage that is not backed up: on iOS, a directory under
`Library/Application Support` marked excluded from backup; on Android, the
no-backup files directory (`Context.getNoBackupFilesDir()`); elsewhere,
exclude it from backup. Do not log records or send them to telemetry.

**Store rules.**

- A store belongs to exactly one swap mnemonic.
- Use one `FileStore` and one `KaleidoSwap` per store directory per process.
- Never reset, copy or restore a store from an older backup while its swap
  mnemonic is in use.
- If a store is lost, start a new swap mnemonic.
- Keep the store directory out of device and cloud backups (iCloud, Android
  Auto Backup), so that a restore never brings back an older copy of it.

To keep swaps in your own storage or derive keys from your own wallet, pass any
object that implements the `SwapStore` interface (`get`, `put`, `list`,
`nextIndex`, records as JSON strings persisted verbatim) or `SwapSigner`
(`publicKey`, `derivePreimage`, `signSchnorr`, `musigNonce`,
`musigPartialSign`, hex in and out; the SDK never asks for a secret key). These are **synchronous**: the native
side calls them and waits for the return value, so they cannot return a
promise. A store of your own follows the store rules above, and:

- `put` does not return until the record is durably stored.
- `nextIndex` persists the advanced counter before it returns, is atomic
  across callers, and never goes backwards.

> **Not yet run on a device.** A JavaScript `SwapStore`, `SwapSigner` and the
> `PhaseListener` passed to `watch` are UniFFI foreign-trait callbacks: native
> code calls back into Hermes. The examples type-check against the generated
> module, and the bindings tests drive a foreign store and signer from Python,
> but the JavaScript callbacks have not been run on an iOS or Android device
> yet. `FileStore` and `MnemonicSigner`
> are Rust and do not cross back into JavaScript. Test your app's swap flow on
> signet on real hardware before you ship it.

**Resume at launch.** A swap has a deadline and a phone does not stay awake.
`resume()` moves every swap the store still holds one step (claiming what is
claimable, refunding what is refundable) and returns a `ResumeReport`:
`resumed` (the `Swap`s still in flight), `settled`, `refunded`, `failed`, and
`errors`, the swaps it could not move this pass, each with an `id`, a `code`
and a `message`. Those stay in the store for the next `resume()`. Follow each
swap in `resumed` to the end with `watch()` or `settled()`; `resume()` does not
wait for funding or confirmations.

### Quote

Both sides are yours to name, each an `Asset`: what it is and where it lives.

| `Asset`                                  | On the wire       |
| ---------------------------------------- | ----------------- |
| `new Asset.BtcOnchain()`                 | `bitcoin:BTC`     |
| `new Asset.BtcLightning()`               | `lightning:BTC`   |
| `new Asset.BtcLiquid()`                  | `liquid:BTC`      |
| `new Asset.UsdtLiquid()`                 | `liquid:USDT`     |
| `new Asset.BtcArkade()`                  | `arkade:BTC`      |
| `new Asset.BtcSpark()`                   | `spark:BTC`       |
| `new Asset.RgbOnchain({ contractId })`   | `bitcoin:rgb:…`   |
| `new Asset.RgbLightning({ contractId })` | `lightning:rgb:…` |
| `new Asset.SparkToken({ tokenId })`      | `spark:btkn…`     |

`contractId` is the full RGB contract id, `rgb:` prefix included.

The protocol follows from the two sides and is reported on the quote, never
inferred from an address. The amount says what it fixes, in sats as a
`bigint`, and each route is priced by one kind:

| Route                                        | Protocol          | `Amount`                                                                         |
| -------------------------------------------- | ----------------- | -------------------------------------------------------------------------------- |
| `BtcLightning` → `BtcOnchain` or `BtcLiquid` | reverse (Boltz)   | `new Amount.Pay(sats)` (what you pay) or `new Amount.Receive(sats)` (what lands) |
| `BtcOnchain` or `BtcLiquid` → `BtcLightning` | submarine (Boltz) | `new Amount.Invoice(bolt11)`: the invoice the maker pays                         |

The wrong kind for a route rejects with `Exception.Swap` (`InvalidAmount` or
`InvalidDestination`) before anything is sent.

```ts
import {
  Amount,
  Asset,
  QuoteRefusalReason,
} from "@kaleidorg/swap-sdk-react-native";

const { quote, refusal } = await kaleido.quote({
  pay: new Asset.BtcLightning(),
  receive: new Asset.BtcLiquid(),
  amount: new Amount.Pay(50_000n),
});
if (!quote) {
  throw new Error(
    `${QuoteRefusalReason[refusal!.reason]}: ${refusal!.message}`,
  );
}
console.log(
  quote.fromAmount,
  quote.toAmount,
  quote.fees.maker,
  quote.fees.miner,
);
```

`quote()` resolves to a `QuoteAnswer` with exactly one of `quote` and `refusal`
set. A refusal is a value, not an error: the maker will not trade this pair or
this amount. `reason` is a `QuoteRefusalReason` (`PairUnavailable`,
`AmountBelowMin`, `AmountAboveMax`, `NoLiquidity`, `MakerPaused`), `limits`
carries the bounds when the amount was the problem, and `message` says why.
Routes this layer does not create swaps for yet (chain swaps, L-USDT, RGB,
Arkade, Spark) are a `PairUnavailable` refusal whose message names where they
live.

### Start, act, watch

```ts
import { Exception, SwapPhase } from "@kaleidorg/swap-sdk-react-native";

const swap = await kaleido.start(quote, {
  destination: liquidAddress,
  refundAddress: undefined,
});
const { invoice, fundingAddress, fundingAmount } = swap.action();

try {
  await swap.watch({ onPhase: (phase) => setPhase(SwapPhase[phase]) });
} catch (e) {
  if (Exception.Swap.instanceOf(e)) showError(e.inner.code, e.inner.message);
  else throw e;
}
```

- `start(quote, { destination, refundAddress })` creates the swap and persists
  it before resolving. `destination` is the address the received asset lands
  on, checked against the asset and the network first; leave it `undefined`
  for a swap priced by an invoice, which is its own destination.
  `refundAddress` is required when you pay from chain and ignored when you pay
  over Lightning.
- `swap.action()` is what the user does: pay `invoice` for a swap from
  Lightning, or send exactly `fundingAmount` sats to `fundingAddress` for a
  swap from chain. That is the only step that is theirs; the claim and the
  refund are the SDK's.
- `swap.watch({ onPhase })` follows the swap to the end, claiming or refunding
  as the maker and the chain move, calls `onPhase` on every change, and
  resolves with the `SwapRecord` on `Settled`. On `Refunded` or `Failed` it
  rejects with `Exception.Swap`. `swap.settled()` is the same without a
  listener. A swap moves only while something watches it (or `resume()`
  runs), which on a phone means while the app is in the foreground: call
  `resume()` at every launch.
- `swap.phase()`, `swap.record()` and `kaleido.swap(id)` read state without
  waiting; `swap.tick()` makes one observation and acts on it.

### Phases

Every protocol moves through one `SwapPhase` machine. The protocol's own status
stays readable as `swap.record().rawStatus`.

```
Created ─▶ AwaitingFunding ─▶ Funded ─▶ Locked ─▶ Claiming ─▶ Settled
                                          │
                                          ├─▶ Refundable ─▶ Refunding ─▶ Refunded
                                          └─▶ Failed
```

`Settled`, `Refunded` and `Failed` are terminal. A swap that becomes
`Refundable` is refunded by the SDK on its own, cooperatively or after the
timeout; `swap.refund()` is there for a caller who wants to decide.

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
