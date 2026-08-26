# `@kaleidorg/swap-sdk`

TypeScript and WebAssembly bindings for KaleidoSwap: Boltz-protocol atomic swaps
(submarine, reverse, and chain) between Bitcoin, Lightning, and Liquid.

## Install

```sh
npm install @kaleidorg/swap-sdk
```

## Usage

`await init()` takes no argument and behaves the same in both runtimes: browsers
resolve the packaged WebAssembly binary relative to the module, and Node reads it
from disk via the `"node"` export condition. Await it once before constructing
any client.

```ts
import { BoltzClient, init } from "@kaleidorg/swap-sdk";

await init();
const boltz = BoltzClient.forNetwork("signet");
const pairs = await boltz.submarinePairs();
```

`"signet"` reaches the live KaleidoSwap maker, so the snippet above runs as
written. `"regtest"` resolves to `http://localhost:9001/v2` and needs this
repository's local harness.

Bundlers must emit the packaged `vendor/bindings_wasm_bg.wasm` asset referenced
by the generated module.

`BoltzClient.forNetwork` resolves the default **KaleidoSwap maker**, which today
serves `"signet"` and `"regtest"` only. `"mainnet"` and `"testnet"` are rejected
rather than silently falling back to a third-party maker — reach one of those by
passing an explicit base URL to `new BoltzClient(baseUrl, timeoutSecs?)`. Signet
settles on Mutinynet, so pair it with Mutinynet chain access
(`https://esplora.signet.kaleidoswap.com`), never a testnet3 endpoint: the two
encode addresses identically, so a mismatch raises no error and simply creates
swaps on one chain while funding or watching another.

## Supplying the binary yourself

To serve the WebAssembly binary from your own CDN, or as a bundler asset URL,
pass any `WasmSource` (a `BufferSource`, `URL`, `Request`, `Response`, or URL
string):

```ts
await init(new URL("/assets/bindings_wasm_bg.wasm", location.origin));
```

`wasmUrl` points at the packaged binary if you need to copy or re-host it. Under
Node you can also pass it straight to `init` — a `file:` source is read from
disk, since Node's `fetch` rejects that scheme. For a pre-compiled
`WebAssembly.Module`, use `initWithModule` — it is separate from `init` so that
`WasmSource` stays type-safe (`WebAssembly.Module` is an empty interface in
TypeScript's lib, so a union containing it accepts any value).

One case needs an explicit source: bundling the Node entry into a single file, as
a CLI or a serverless artifact does, moves `import.meta.url` away from the
packaged `vendor/`, so the default lookup has nothing to find. Copy
`bindings_wasm_bg.wasm` next to your output and pass it — that failure surfaces
at runtime rather than at build time, so reach for this before you ship a bundle.

SDK operations also require the web APIs used by the selected client, including
`fetch` and WebSocket. Node 22 and newer provide both.

## Swap keys

Swap keys and preimages derive client-side from a wallet mnemonic via BIP85
index 26589 — no key material leaves the caller:

```ts
import { SwapMasterKey } from "@kaleidorg/swap-sdk";

const master = SwapMasterKey.fromWalletMnemonic(walletMnemonic, "regtest");
const { publicKey, secretKey } = master.deriveSwapKey(0n);
```

## Typed surface

- `BoltzClient` — Boltz swap API (create submarine/reverse/chain swaps, pairs,
  fees, quotes, restore).
- `BoltzWsApi` / `BoltzWsUpdates` — WebSocket swap-status stream.
- `SwapScript` — reconstruct a swap from its creation response, then build
  claim/refund transactions (`constructClaim`, `constructRefund`) or
  caller-funded Liquid PSETs (`prepareLiquidClaim`, `prepareLiquidRefund`).
  Chain swaps claim through `constructCooperativeClaim` instead — the cheaper
  MuSig2 keyspend, partial-signed with the swap's **refund** key rather than its
  claim key. `constructClaim` cannot carry the lockup script that path signs
  against, so a chain swap taking the script path must pass
  `cooperative: false`.
- `SwapMasterKey` — BIP85 swap key and preimage derivation.
- `isKaleidoSwapError` — narrow a rejection to its stable `code`.

Boltz request and response payloads are Rust-defined and cross the boundary as
plain objects typed `any`; the rest of the surface is hand-typed. The packaged
`dist/index.d.ts` carries the full signatures and their documentation.

Every rejection produced after an argument reaches the Rust binding is an `Error`
carrying a `code`. Input the binding rejects — a mistyped string argument, an
unparseable key or preimage, or a request object missing a required field — uses
`InvalidArgument` and names the argument or field. Failures from the swap engine
carry their own code; binding-internal failures use `Internal`.

Values rejected earlier by wasm-bindgen's generated ABI glue remain native
JavaScript errors. In particular, passing a `number` where a declared `bigint` is
required throws `TypeError` before Rust can attach a code.

## Partner attribution — `createKaleidoMakerClient`

A partner organization can have the swaps it originates attributed to it. That
needs an **organization API key** from the KaleidoSwap partner panel — a
`kld_test_…` key for signet and staging, `kld_live_…` for mainnet and
production. Without one, `BoltzClient` behaves exactly as before and creates
unattributed swaps.

```ts
import { init, createKaleidoMakerClient } from "@kaleidorg/swap-sdk";

await init();
const client = createKaleidoMakerClient({
  makerUrl: "https://maker.signet.kaleidoswap.com/v2",
  apiKey: process.env.KALEIDOSWAP_API_KEY!,
});

client.apiKeyEnvironment; // "test"
client.apiKeyId; // the key id the partner panel shows
```

The result is an ordinary `BoltzClient` — every route works the same way — that
sends the key as `Authorization: Bearer …` on requests to `makerUrl`, and only
to `makerUrl`. The key answers _which partner organization created this swap?_
and nothing else: it authorizes no claim, no refund, no fund movement and no
panel access. The per-swap `swapAuth` below stays separate and unchanged.

`makerUrl` must be `https` unless it is a loopback address, since a bearer
credential over plain HTTP is readable by anything on the path. A value that
cannot be a key is rejected here rather than reaching the maker as a `401` —
which is the same answer a revoked key gets, so a local typo would otherwise
read as a suspended organization. There is no accessor for the secret half:
`apiKeyId` and `apiKeyEnvironment` are all JS can read back.

> **Do not ship an organization key to a browser.** It is permanent until
> revoked, with no origin binding and no per-key rate limit, so a key in a
> bundle is visible to every visitor — who can then attribute their own swaps
> to, or exhaust the limits of, an organization that is not theirs. This release
> supports **server and native integrations only**: call this from Node, keep the
> key in server-side configuration, and leave browser code on the plain
> `BoltzClient` constructor.

One protection is also weaker under `fetch` than on a server. `fetch` owns
redirect handling and the SDK can set no policy on it, so a `3xx` away from the
maker is reported after the fact instead of declined: the call fails naming the
host that answered. The key itself is not disclosed by such a hop — `fetch` drops
`Authorization` when a redirect crosses origins — but nothing that response says
came from the maker.

## `swapAuth` — persist it with the swap

Every create response from the KaleidoSwap maker carries a `swapAuth`: a
per-swap credential, issued **once**, that authorizes accepting a chain-swap
re-quote. Store it with the swap, and treat it as secret — it is the taker's
full capability over that swap. It is a plain property of the create response,
so `console.log(swap)` puts it in your logs; log `swap.id` instead.

```ts
import { toJson } from "@kaleidorg/swap-sdk";

const swap = await client.createChainSwap("regtest", req);
// `toJson`, not `JSON.stringify`: a create response carries bigint amounts that
// `JSON.stringify` throws on, and the throw would land on the one step whose
// whole purpose is not losing the credential. See "Lossless integer values"
// below; `swapAuth` is a plain string and survives either way.
await store.put(swap.id, toJson(swap)); // swapAuth included — never re-issued

// Later, possibly in another session:
const saved = JSON.parse(await store.get(swapId));
const quote = await client.quote(swapId);
await client.acceptQuote(swapId, quote.amount, saved.swapAuth);
```

Accepting a re-quote commits the maker's payout at the re-quoted amount, so the
maker authorizes it with the credential rather than with the swap id — the id
travels through status polls, `/v2/ws`, webhooks and logs, and is no secret.
Without it the call is rejected `401 invalid_swap_auth`, and no other route
resolves the re-quote: the swap sits until it expires into its refund path.

Reading a re-quote (`quote()`) needs no credential, so seeing one says nothing
about being able to accept it.

Nothing re-issues a lost `swapAuth` — `swapRestore()` authenticates with an XPUB
alone and does not return it. Recovery is an operator action, not a client one.

Pass `undefined` only for a maker that issues none: upstream Boltz declares no
auth on this route, which is why the field is optional on both sides.

## Lossless integer values

Amounts cross the WASM boundary as `bigint`. Use the exported `toJson` helper
when serializing SDK responses:

```ts
import { toJson } from "@kaleidorg/swap-sdk";

console.log(toJson({ amount: 1000n }));
```

This applies to arguments as well as responses, and the declared type is the rule
to follow — it differs by where the value crosses the boundary.

A parameter declared `bigint` is passed on the wasm-bindgen ABI, which accepts a
`BigInt` and nothing else, so it needs the `n` suffix:

```ts
master.deriveSwapKey(0n); // ok
master.deriveSwapKey(0); // TypeError: Cannot convert 0 to a BigInt
```

`tsc` rejects the plain-number form ahead of that throw. The same applies to any
other `bigint` argument, such as the `BoltzClient` constructor's `timeoutSecs`.

Fields inside request objects are declared `number` even where the Rust type
behind them is 64-bit, because those objects are deserialized rather than passed
on the ABI. Pass what the field declares and both cases are correct.

## Arkade Intents venue (`@kaleidorg/swap-sdk/arkade`)

An optional subpath serving the Arkade Intents RFQ routes
(`arkade:BTC ↔ lightning:BTC`) against any solver card. Opt-in by design:
it peer-depends on `@arkade-os/sdk` and `@arkade-os/swap`, which a
Boltz-only consumer never installs.

```ts
import {
  ArkadeIntentsVenue,
  InMemoryArkadeSwapStore,
} from "@kaleidorg/swap-sdk/arkade";

const venue = new ArkadeIntentsVenue({
  wallet,
  arkServerUrl,
  transport,
  store,
});
const { address, fundAmountSats } = await venue.prepareLightningSend({
  invoice,
});
// The recovery record is persisted BEFORE this returns. Funding is the
// quote acceptance:
const txid = await wallet.send({ address, amount: fundAmountSats });
await venue.notifyFunded(record.id, txid);
```

Drive `venue.reconcile()` from your own scheduler (MV3 `chrome.alarms`, a
node interval) — one evidence-driven pass that claims funded receives,
refunds matured sends, and resolves records from chain evidence. The venue
owns no timers and trusts no relay status message.

## Development checks

Build fresh WASM bindings from the repository root before running the package
checks:

```sh
make wasm-pack-build
cd typescript-sdk
npm ci
npm run typecheck
npm run lint
npm run format:check
npm test
npm run smoke:package
npm run smoke:browser-package
```

`smoke:browser-package` loads the packaged browser entry in headless Firefox, so
it needs `firefox` on `PATH` — point `BROWSER_BIN` at the binary if it lives
elsewhere (on macOS,
`/Applications/Firefox.app/Contents/MacOS/firefox`). Both smoke scripts pack a
throwaway tarball when given no argument, or check a supplied one:
`npm run smoke:package -- path/to/package.tgz`.
