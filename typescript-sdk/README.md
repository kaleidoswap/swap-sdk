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
import { SwapClient, init } from "@kaleidorg/swap-sdk";

await init();
const client = SwapClient.forNetwork("signet");
const pairs = await client.submarinePairs();
```

`"signet"` reaches the live KaleidoSwap maker, so the snippet above runs as
written. [`examples/`](examples/README.md) carries that further: four runnable
programs, two of which need nothing but Node.

```sh
cd examples && npm install && node 01-pairs.ts
```

`"regtest"` resolves to `http://localhost:9001/v2` and needs this
repository's local harness.

## Lightning addresses and LNURL-pay

```ts
import { init, resolveLightningAddress } from "@kaleidorg/swap-sdk";

await init();
const invoice = await resolveLightningAddress("alice@example.com", 25_000n);
// Pass this fixed-amount BOLT11 to the normal submarine-create flow.
```

The resolver also accepts an encoded `lnurl1...` pay request (including uppercase
encoding). `amountSat` is a positive **bigint in whole satoshis**; unsafe types,
zero, negative values and amounts that overflow the millisatoshi conversion are
rejected. It reuses the Rust LNURL resolver, checks the provider's advertised
amount limits, parses the returned BOLT11 and requires its exact amount and LNURL
metadata description hash to match.
Service and transport errors reject the promise. Native/Python exposes
`await resolve_lightning_address(address, amount_sat)` with the same satoshi unit.

Lookup requires HTTPS for clearnet service and callback URLs; HTTP is accepted
only for onion services (native callers must provide Tor connectivity). Redirects,
URL credentials and fragments are rejected. Each request, including body reads,
has a 30-second deadline in native and browser/Node WASM. It creates no swap
and sends no payment or maker credentials. Browser providers must allow CORS. The resolver does not select or pin the invoice
network or check expiry; callers must apply their invoice and provider trust
policy before using the result. Do not
interpret successful lookup as payment settlement.

When the `lnurl` feature is enabled, HTTPS now uses Rustls with WebPKI roots,
matching the SDK transport and avoiding system OpenSSL dependencies in wheels.
An enterprise CA installed only in the OS trust store is not automatically trusted.

## Fee rates and tip heights for claims and refunds

Use an explicitly configured Esplora endpoint for the chain and network the
transaction spends on. The pure TypeScript helpers require no WASM initialization
and are also exported from the main package:

```ts
import {
  esploraFeeEstimates,
  selectEsploraFeeRate,
  esploraTipHeight,
} from "@kaleidorg/swap-sdk/chain-data";

const baseUrl = "https://your-bitcoin-esplora.example/api";
const [fees, tipHeight] = await Promise.all([
  esploraFeeEstimates(baseUrl, { timeoutMs: 10_000 }),
  esploraTipHeight(baseUrl, { timeoutMs: 10_000 }),
]);
const feeRateSatVb = selectEsploraFeeRate(fees, 2); // Target: two blocks; sat/vB.
if (feeRateSatVb === undefined)
  throw new Error("No estimate for the requested target");
```

Fee estimates map positive confirmation targets to positive, finite **sat/vB**
rates; fractional Liquid rates are preserved. Missing targets stay `undefined`.
The tip is an integer block height. An Esplora genesis tip of zero is valid;
missing, failed or malformed tip data rejects rather than producing zero.
The fetch helpers make one request per call and do not cache, retry, select
another target, substitute a fixed fee or switch endpoints. The default deadline
is 30 seconds, including reading the response body; `signal` supports cancellation
and `fetch` supports injected transports. HTTP errors report their status via
`EsploraDataError`, separately from malformed successful responses. Network and
cancellation errors propagate. Requests omit cookies and maker credentials and
refuse redirects. A configured URL that redirects (including HTTP→HTTPS)
fails as a network error; configure the final URL directly.

An empty `{}` fee map is valid when the explorer has no estimates yet.
`selectEsploraFeeRate(fees, target)` chooses the largest available target at or
below the requested block count (equally fast or faster). Missing eligible
estimates return `undefined`; it never substitutes a slower target or a default
fee. Fractional Liquid rates remain fractional. Postpone construction or apply
an explicit application policy when no eligible estimate exists.

Select the endpoint alongside the spend's existing Esplora configuration. Bitcoin,
Liquid, testnet and regtest are distinct, and Mutinynet is not vanilla signet.
The endpoint is trusted chain data; these helpers do not identify its network or
prove relay acceptance. If it is unavailable or has no requested target, postpone
construction or apply an explicit application policy using a verified source on
the same network. Do not silently turn a failure into a usable height or fee.

The existing maker-backed methods remain useful for maker pricing and diagnostics:

| Source                   | Bitcoin                                                  | Liquid                                       |
| ------------------------ | -------------------------------------------------------- | -------------------------------------------- |
| `client.feeEstimation()` | Node's two-block estimate, floored to 1 sat/vB when zero | Fixed 0.1 sat/vB policy; not a live estimate |
| `client.height()`        | Node tip                                                 | Provider tip, or **0** when unavailable      |

Treat the maker's Liquid zero as unavailable; it cannot establish refund maturity.
For client-built claims/refunds, use the explicit Esplora policy above rather than
falling back to that placeholder or the maker's fixed fee. Selecting a fee rate
does not prove that a particular transaction will relay. Liquid relay validation
must cover the intended network and transaction types separately.

## Stateless claim/refund reconciliation

The TypeScript helpers `canRefund`, `canClaim`, `refundSubmarineSwap`,
`refundChainSwap`, `claimReverseSwap` and `claimChainSwap` compose the existing
validated `SwapScript` constructors. They return a signed transaction or
`{ kind: "not-yet", reason }`; they keep no state, run no timers and do not
broadcast. Rust/native retain the lower-level constructors.

```ts
import { refundSubmarineSwap } from "@kaleidorg/swap-sdk";

const result = await refundSubmarineSwap({
  status, // fresh maker status for this swap
  tipHeight, // trusted tip of the chain being refunded
  createResponse, // original verified response / timeout
  script, // validated submarine script for this same swap
  params: {
    outputAddress,
    swapId,
    keysSecretHex,
    swapAuth,
    network,
    makerBaseUrl,
    bitcoinEsploraUrl,
    feeSatPerVb: 2, // choose explicitly; fractional Liquid rates work
  },
});
if (result.kind === "signed") {
  // Persist { swapId, action, path, hex, txid, chain } BEFORE broadcasting.
  // Store secrets separately. Application storage/scheduling are your own.
  const hex = result.transaction.hex();
  const txid = result.transaction.txid();
  // await saveSpend(...);
  // await result.transaction.broadcast(network, bitcoinEsploraUrl, liquidEsploraUrl);
}
```

Refund `mode: "auto"` attempts authenticated cooperation on failed/expired swaps
before the timeout, and uses the script path once the lockup chain tip reaches
`timeoutBlockHeight`. At maturity, stale `swap.created`, `invoice.pending` and
unknown maker states cannot veto a unilateral refund. States indicating payment,
claiming or a completed claim remain blocked. UTXO availability and consensus
still decide whether a refund can be mined. `mode: "cooperative"` always requires
the maker, and **refund** refusals propagate without a fallback. Local eligibility
cannot authorize a maker signature or bypass its financial rules (#674).

Claims require independently verified receiving-side chain evidence:
`lockup: { chain: "bitcoin" | "liquid", hex, txid, confirmations }`. Obtain raw
bytes, transaction ID and confirmation count from the application's trusted
chain backend, verify the swap output, and refresh the evidence before signing.
The maker's status is not confirmation proof. Missing evidence or fewer than
`minConfirmations` (default 1) yields `not-yet` before any preimage disclosure.
The WASM constructor verifies the raw transaction's txid and pins input selection
to that transaction; it neither discovers a different mempool HTLC nor
rebroadcasts the lockup. `allowZeroConf: true` explicitly accepts an unconfirmed
lockup, but still requires its raw bytes and txid.

Claims stop before the deadline by `claimSafetyMarginBlocks`: default 6 blocks
for Bitcoin and 60 for Liquid. Override this policy explicitly; 0 accepts an
expiry race. These margins cannot guarantee confirmation in time. Chain swaps
use **lockup** tip/timeouts for refunds and **claim** tip/timeouts for claims.
Pass the full response with `lockupDetails`/`claimDetails` or the appropriate
side's details. Snapshots can be stale or reorged; construction/broadcast errors
still propagate, and a signed result must be saved and broadcast promptly.

Reverse claims in `mode: "auto"` prefer cooperation. Chain claims prefer it when
both `lockupScript` and the distinct `refundKeysSecretHex` are provided. If a
cooperative claim fails, auto mode immediately builds the unilateral script
claim against the same pinned lockup: the maker may already know the preimage.
Explicit `mode: "cooperative"` propagates failures, and callers must urgently
switch to `mode: "script"` after a refusal. If both paths fail, the coded
`ClaimFallbackFailed` error carries both causes and requires urgent script-path
recovery. There is no automatic broadcast or network retry.

Every construction needs exactly one positive `feeSatPerVb` or integer
`feeAbsoluteSat`. Optional `feeLimits: { maxFeeSatPerVb, maxFeeAbsoluteSat }`
rejects an excessive configured fee before signing. No maker fee default is
assumed. The explicit Esplora helpers from [PR #112](https://github.com/kaleidoswap/swap-sdk/pull/112)
provide fee estimates and tip heights; lockup confirmation/status and raw bytes
must also come from the trusted chain source. Restore scripts using the
validated create response or `SwapScript.fromRestored` from [PR #111](https://github.com/kaleidoswap/swap-sdk/pull/111).

To retry a lost broadcast, pass the saved/recovered `SignedSwapSpend` as
`previous`. The helper returns that exact transaction before inspecting new
status, height or fees, and rejects a different swap ID/action. After restart,
rebuild `previous.transaction` with `BtcLikeTransaction.fromHex(saved.hex,
saved.chain)` from [PR #109](https://github.com/kaleidoswap/swap-sdk/pull/109),
retaining the recorded chain/action/path. These companion APIs are delivered by
their separate PRs and must be included in the installed SDK release. Never re-sign simply because
a broadcast response was lost. Apps must serialize concurrent operations per
swap and durably retain the first signed result; a stateless helper cannot
deduplicate two concurrent calls without shared saved state. Treat duplicate
broadcast responses as successful only after independently checking that the
original txid is known on the intended chain.

Bundlers must emit the packaged `vendor/bindings_wasm_bg.wasm` asset referenced
by the generated module.

`SwapClient.forNetwork` resolves the default **KaleidoSwap maker**, which today
serves `"signet"` and `"regtest"` only. `"mainnet"` and `"testnet"` are rejected
rather than silently falling back to a third-party maker — reach one of those by
passing an explicit base URL to `new SwapClient(baseUrl, timeoutSecs?)`. Signet
settles on Mutinynet, so pair it with Mutinynet chain access
(`https://esplora.signet.kaleidoswap.com`), never a testnet3 endpoint: the two
encode addresses identically, so a mismatch raises no error and simply creates
swaps on one chain while funding or watching another.

## Pay-through

`@kaleidorg/swap-sdk/pay-through` is a small HTTP client for the maker's
`POST /v2/swap/pay` and `GET /v2/swap/{id}`. It needs no WASM initialization.
The maker must enable `swap.pay_through_enabled` (off by default), and the
caller still pays the returned Lightning invoice with its own wallet.

```ts
import { PayThroughClient } from "@kaleidorg/swap-sdk/pay-through";

const pay = new PayThroughClient({
  makerUrl: "https://maker.signet.kaleidoswap.com/v2",
});
const order = await pay.create({
  destination: recipientAddress,
  invoiceAmount: 100_000,
});
// Show order.invoice, order.payoutAsset, and order.payoutAmount for approval.
// After the wallet explicitly pays order.invoice:
const status = await pay.status(order.id);
```

The payer's Lightning payment remains held until the maker broadcasts the
address payout. The address payout has no hash lock: the maker controls that
leg. `transaction.mempool` reports broadcast, and `invoice.settled` reports
settlement of the hold. Store `id` and `swapAuth` privately before paying the
invoice.

`create` refuses terms that differ from the request: another destination, a
different `invoiceAmount`, less than the requested `payoutAmount`, or another
`asset`. It also decodes the invoice and refuses one whose amount or payment
hash differs from `invoiceAmount` and `paymentHash`. Amounts use safe JS
integers, and a larger response is rejected rather than rounded.

Requests time out after `timeoutMs` (30 s by default) and take an optional
`signal`. A `PayThroughApiError` with a 4xx status means nothing was created;
a 5xx, a timeout or a network error leaves the outcome unknown. There is no
idempotency key, so do not blindly retry creation or pay a second invoice.

## Arkade Intents corridor

`ARKD` in the maker's catalogue is bitcoin on Arkade, and it is not a
Boltz-shaped route — `createReverseSwap({ to: "ARKD" })` is refused before any
I/O. The maker serves it as an RFQ beside `/v2`, and `IntentsCorridor` speaks
that wire over the same client:

```ts
import {
  SwapClient,
  IntentsCorridor,
  isRfqQuote,
  newRfqId,
} from "@kaleidorg/swap-sdk";

const corridor = new IntentsCorridor(SwapClient.forNetwork("signet"));
const answer = await corridor.quoteLightningReceive({
  rfq_id: newRfqId(),
  amount_side: "to",
  amount: 12_000, // receive exactly this on Arkade; the price is the free variable
  payment_hash, // sha256 of YOUR preimage, hex
  payout_address, // your Ark address
  payout_pubkey, // your x-only Ark key, hex
});
if (!isRfqQuote(answer)) throw new Error(`refused: ${answer.reason}`);
// answer.profile.invoice is the hold invoice to pay; verify it pays payment_hash
// for exactly answer.from_amount before handing it to a payer.
```

Payloads keep the wire's snake_case and `bigint` amounts on purpose: a quote
from here is the `RfqQuote` that `@arkade-os/swap`'s gates and derivations
take. Funding or claiming the Arkade side needs an Ark wallet — that is the
`@kaleidorg/swap-sdk/arkade` venue, whose transport you build from the same URL
with `kaleidoswapHttpTransport("https://maker.signet.kaleidoswap.com/v2")`.

Discovery cards are **unverified**. `corridor.card()` / `client.corridorCard()`
reject unsupported versions (only v0 is understood) and discard unknown fields.
For verification, use `corridor.cardRaw()` / `client.corridorCardRaw()` (Python:
`corridor_card_raw()`): the exact response text preserves unknown fields, member
order and numeric spelling. The maker signs SHA256 of its compact JSON card
**without `sig`**, in the maker's serialization order. Do not hash the entire
response or reserialize the typed record. Use lossless ordered JSON processing
and preserve every field; ordinary JS JSON parsing can round future large
integers. Check the Schnorr signature against `discovery_pubkey` and establish
trust in that identity separately before acting on advertisements. A valid
signature alone is not an endorsement. Each fetch is a separate observation;
verify the raw card you intend to use rather than combining two fetches.

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

## Recovering a saved transaction

Persist the signed transaction's hex, chain, network and Esplora endpoint
**before** broadcasting. After a restart or a lost broadcast response, rebuild
the transaction and explicitly rebroadcast the same bytes:

```ts
import { BtcLikeTransaction, init } from "@kaleidorg/swap-sdk";

await init();
// Load these from the receipt saved before the original broadcast.
const tx = BtcLikeTransaction.fromHex(receipt.hex, receipt.chain);
const txid = await tx.broadcast(
  receipt.network,
  receipt.chain === "bitcoin" ? receipt.esploraUrl : undefined,
  receipt.chain === "liquid" ? receipt.esploraUrl : undefined,
);
```

`receipt.chain` is `"bitcoin" | "liquid"`; network selection is separate.
Native/Python uses `BtcLikeTransaction.from_hex(hex, TransactionChain.BITCOIN)`
or `TransactionChain.LIQUID`. These select the encoding family only: transaction
bytes do not identify mainnet, testnet or regtest. Keep the network in the saved
receipt and select it separately when broadcasting.
The factory performs local decoding only: it does not sign, select fees or
inputs, query the maker, or broadcast. Malformed, truncated, trailing-byte and
noncanonical encodings raise an `InvalidArgument` error. It preserves all
accepted transaction bytes, including witnesses; uppercase hex is accepted
and `hex()` returns lowercase. Store the original txid too if you want to
verify the recovered receipt before sending it.

There is no automatic retry. A lost response does not prove broadcast failed;
check the original txid at the same chain backend and retain the original
transaction. Rebroadcasting identical bytes cannot create another transaction,
but a backend may report that it already knows the transaction. This helper
does not turn that rejection into success or validate that a transaction is
spendable. Never construct a new claim/refund just to retry an uncertain send.

## Swap keys

Swap keys and preimages derive client-side from a wallet mnemonic via BIP85
index 26589 — no key material leaves the caller:

```ts
import { SwapMasterKey } from "@kaleidorg/swap-sdk";

const master = SwapMasterKey.fromWalletMnemonic(walletMnemonic, "regtest");
const { publicKey, secretKey } = master.deriveSwapKey(0n);
```

## Typed surface

**Renamed in 0.9.0.** `BoltzClient`, `BoltzWsApi` and `BoltzWsUpdates` are now
`SwapClient`, `SwapWsApi` and `SwapWsUpdates`, and `TxParams` /
`LiquidPsetParams` take `makerBaseUrl` and `makerTimeoutSecs` in place of
`boltzBaseUrl` and `boltzTimeoutSecs`. The old names are gone rather than
deprecated — see [`VERSIONING.md`](../VERSIONING.md). Passing the old field
name fails naming both spellings.

The client speaks the Boltz protocol and can be pointed at Boltz's own API;
that provenance is recorded in the
[root README](../README.md#the-swap-engine-boltz-protocol) and the LICENSE,
which is where it belongs rather than in a symbol an integrator types.

See the [maker/upstream support matrix](../docs/compatibility.md) for the
deliberately retained Rust-only raw routes and the existing native submarine
co-sign helper. `constructCooperativeClaim` is a chain-claim constructor; it does
not expose submarine co-signing. Maker-mediated raw broadcast and Boltz-shaped
BOLT12 management are not exported here and are absent from the KaleidoSwap
public router.

- `SwapClient` — the swap API (create submarine/reverse/chain swaps, pairs,
  fees, quotes, restore).
- `SwapWsApi` / `SwapWsUpdates` — WebSocket swap-status stream.
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

`swap()`, the three create methods, `swapRestore()` and `swapRestoreIndex()` use
public DTOs generated from the Rust/serde definitions. Types such as
`CreateReverseRequest`, `CreateReverseResponse`, `GetSwapResponse` and
`SwapRestoreResponse` can be imported from this package. Node, browser and raw
WASM declarations carry the same types; the packaged `dist/index.d.ts` also
documents the hand-written helpers.

Restore entries carry the literal `type` discriminator (`"submarine"`,
`"reverse"` or `"chain"`), with optional claim/refund details. A missing or null
Rust `Option` becomes `undefined` in responses. Maker status responses preserve
payout progress and payment audit events. Event `data` is `unknown`: narrow it
before reading fields. JSON integer encodings inside `data` arrive as `bigint`, including
small values; floating-point encodings arrive as `number` (`2` becomes `2n`,
while `2.5` stays `2.5`). Narrow for both
forms as appropriate. These values are not JSON-serializable as-is:
`JSON.stringify(event.data)` can throw; use `toJson(event.data)` for persistence.
Native bindings expose that open audit value as JSON text.

Every rejection produced after an argument reaches the Rust binding is an `Error`
carrying a `code`. Input the binding rejects — a mistyped string argument, an
unparseable key or preimage, or a request object missing a required field — uses
`InvalidArgument` and names the argument or field. Failures from the swap engine
carry their own code; binding-internal failures use `Internal`.

Values rejected earlier by wasm-bindgen's generated ABI glue remain native
JavaScript errors. In particular, passing a `number` where a declared `bigint` is
required throws `TypeError` before Rust can attach a code.

## USDT-RGB swaps

`createSubmarineSwap(network, request, rgbContractId)` supports `USDT-RGB → BTC`;
`createReverseSwap(network, request, rgbContractId)` supports `BTC → USDT-RGB`.
The third argument pins the expected contract locally. It is required for RGB
before any POST and is not sent to the maker; BTC/Liquid calls may omit it.
Pair cards do not identify the contract. RGB chain swaps are unsupported.

The wallet uses rgb-lib to fund a submarine HTLC, or accept the maker's lock
consignment for a reverse swap. The SDK validates the Bitcoin HTLC and RGB lock
instructions, but rgb-lib validates the asset proofs. `rgb.amount` is in contract
units (six decimals for USDT-RGB); `rgb.htlcSat` is Bitcoin satoshis.

Reconstruct the validated response using `SwapScript.fromReverse` or
`fromSubmarine`, then call `prepareRgbClaim` or `prepareRgbRefund` with:

```ts
const spend = await script.prepareRgbClaim({
  outputAddress: rgbWalletReceiveAddress,
  funding: { kind: "htlcValue", feeRateSatVb: response.rgb.claimFeeRate },
  maxFee: 10_000n,
  swapId: response.id,
  makerBaseUrl,
  network: "regtest",
  bitcoinEsploraUrl,
  lockupTxHex,
});
```

The payout address must correspond to an RGB wallet witness receive. For a
caller-funded refund, use `{ kind: "callerInputs" }`, add BTC inputs/change, and
retain the new spend from `spend.fund(fundedPsbt)`. Its `template()` contains the
frozen funded PSBT and current HTLC input index. Fund before coloring or signing.

Pass the template to rgb-lib's `psbt_op_prepare_with_expiry`, assigning the asset
amount to `paymentOutputIndex`, then return its PSBT and actual allocations to
`finalizeClaim(coloredPsbt, keysSecretHex, preimageHex)` or
`finalizeRefund(coloredPsbt, keysSecretHex)`. These return a signed PSBT and an
optional `BtcLikeTransaction` (`null` while wallet inputs remain unsigned).
Preserve the HTLC witness when signing the remaining wallet inputs. Free spend
and transaction handles after use.

The wallet retains its operation ID and completes `psbt_op_mark_broadcast`,
broadcast, `psbt_op_apply` and `psbt_op_provide_receive_consignment`. A Bitcoin
broadcast alone does not complete the RGB wallet receive. Reverse swaps have no
`transaction.confirmed` event: enforce the lock confirmations through the chain
and rgb-lib before claiming. Refunds wait for the timeout; claims must confirm
before it. Insufficient HTLC sats produce `rgb_fee_input_required`; prepare with
caller inputs instead.

See [the wallet adapter example](examples/05-rgb-spend.ts) and
[the RGB design](../docs/rgb-swaps-plan.md). The adapter is a coordination example;
the [native regtest example](../examples/rgb-regtest/README.md) records actual RGB wallet validation.

Submarine BTC collateral is capped at 1000 sats by default, independently of
`maxFee`/`max_fee`. The maker receives all of it on a successful claim. Choose
any larger cap locally before reading the response: pass `rgbMaxHtlcSat` as the fourth argument to `createSubmarineSwap`.
The cap is never sent to the maker. RGB spend preparation requires the actual
colored lock transaction; address discovery can select unrelated BTC outputs.
Before paying a reverse invoice, compare its `claimFeeRate` with your current
fee estimates and remaining timeout; reject an inadequate quote. If fees rise,
use a higher spend rate and wallet BTC inputs when needed. A maker fee quote
does not guarantee confirmation before the timeout. Contract-id chunk dashes
are cosmetic; pin a valid id from your trusted wallet or asset registry.

## Partner attribution — `createKaleidoMakerClient`

A partner organization can have the swaps it originates attributed to it. That
needs an **organization API key** from the KaleidoSwap partner panel — a
`kld_test_…` key for signet and staging, `kld_live_…` for mainnet and
production. Without one, `SwapClient` behaves exactly as before and creates
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

The result is an ordinary `SwapClient` — every route works the same way — that
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

> **This throws in a browser.** An organization key is permanent until revoked,
> with no origin binding and no per-key rate limit, so a key in a bundle is
> visible to every visitor — who can then attribute their own swaps to, or
> exhaust the limits of, an organization that is not theirs. This release
> supports **server and native integrations only**, and because one wasm
> artifact serves both runtimes the check is a runtime one: a document context
> is refused unless you pass `allowBrowser: true`. Call this from Node, keep the
> key in server-side configuration, and leave browser code on the plain
> `SwapClient` constructor.

> **Scrub the key in error reporters that capture locals.** It crosses the wasm
> boundary as a plain `string`, so it is a function argument on a stack frame
> for the length of the call. The SDK keeps it out of its own errors, logs and
> `toString`, but a reporter configured to capture local variables — Sentry's
> `includeLocalVariables`, and its equivalents — reads it off the frame
> regardless. Drop `apiKey` in your before-send hook.

One protection is also weaker under `fetch` than on a server. `fetch` owns
redirect handling and the SDK can set no policy on it, so a `3xx` away from the
maker is reported after the fact instead of declined: the call fails naming the
origin that answered. The key itself is not disclosed by such a hop — `fetch`
treats any change of scheme, host or port as cross-origin and drops
`Authorization` — but nothing that response says came from the maker. The advice
in that error is written for native clients, whose stripping rule ignores the
scheme, so on a scheme-only hop it will tell you to revoke a key `fetch` had in
fact already dropped. Erring that way round is deliberate.

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
other `bigint` argument, such as the `SwapClient` constructor's `timeoutSecs`.

64-bit fields inside swap request objects accept `bigint | number`, because
serde deserializes those objects. Use BigInt for values above
`Number.MAX_SAFE_INTEGER`; unsafe Numbers are rejected before sending the create
request. Optional request fields also accept `null`.

64-bit response fields are always `bigint`, including amounts, event timestamps,
`createdAt` and the restore index. Rust `u32`/`u8` fields remain `number`, such as
restore key indices, output indices and reverse/chain timeout heights. Submarine
timeout height is a Rust `u64` and therefore `bigint`.

## Swap DTO generation and maker schema checks

From the repository root, run `make generate-swap-types` after editing a Rust DTO.
`make check-swap-types` regenerates the declarations with pinned `ts-rs` and
rejects differences. Package CI runs this check and
`node typescript-sdk/scripts/check-swap-schema.mjs` against the committed
`tests/fixtures/maker-swap-schema.json` snapshot. The snapshot records the exact
maker commit and source file; the checker validates routes, supported fields,
required fields, references, integer representations and restore discriminators. Its explicit exceptions
cover upstream Boltz extensions and maker routes this SDK does not expose, such
as BOLT12 offer creation and legacy flat/Arkade chain responses.

To refresh or check the upstream contract, authenticate `gh` with read access to
the private `kaleidoswap/kaleidoswap-maker-rs` repository, then run:

```bash
uv run scripts/update_maker_swap_schema.py --check
uv run scripts/update_maker_swap_schema.py --revision main
node typescript-sdk/scripts/check-swap-schema.mjs
```

Review snapshot changes together with the Rust DTOs and any compatibility
exceptions. CI always validates the pinned snapshot. Live upstream drift
detection is a separate, owner-triggered **Maker schema drift** workflow. It
checks trusted default-branch scripts and scopes `MAKER_SCHEMA_READ_TOKEN`
(read-only maker Contents access) to its single live-check step. Required PR
builds never receive that token or depend on moving maker `main`; they always
check the pinned contract. Run the manual workflow after deployment/schema
changes or refresh locally with authenticated read access as above.

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

## RGB cooperative submarine refunds

A maker advertising `response.rgb.cooperativeRefund === "rgb-coop-refund-v1"`
can co-sign an early refund using the existing submarine `/refund` endpoint.
The caller's RGB wallet owns funding, coloring and operation recovery:

```ts
import { getRgbRefundPartialSig } from "@kaleidorg/swap-sdk";

const prepared = await script.prepareRgbCooperativeRefund(params);
// Add native SegWit BTC fee inputs if required; color the frozen template
// with your trusted RGB wallet and persist its operation before signing.
const session = prepared.beginCooperativeRefund(
  colored,
  refundKey.secretKey,
  swap.id,
);
const reply = await getRgbRefundPartialSig(
  client,
  swap.id,
  session.request(),
  swap.swapAuth,
);
const finalized = session.complete(reply, refundKey.secretKey);
// Wallet signs remaining fee inputs, persists the signed PSBT/transaction,
// broadcasts and reconciles the RGB operation and receive consignment.
```

`complete` permits one completion; a cryptographic verification failure also
consumes the session. An identical request can
recover the maker's stored reply; after a client restart, create a fresh session
for the same persisted colored transaction. Secret nonce material is never
serialized. The SDK verifies allocations from the trusted local wallet; the
maker independently validates the actual RGB proof and chain prevouts.

The SDK refuses this path when the capability is absent. The existing colored
CLTV refund remains the escape path; changing locktime creates another
transaction and requires a new wallet operation after safe reconciliation.
V1 supports submarine refunds only, one colored transaction per swap, and no
cooperative RBF or automatic fallback after ambiguous broadcast.

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

### Recovering swap authorization

Discover swaps with `SwapMasterKey.restore(client, gapLimit?)` and the highest
known key index with `SwapMasterKey.restoreIndex(client, gapLimit?)`. Both use
the account xpub returned by `masterXpub()` with derivation path `"m"`, matching
`deriveSwapKey(index)`. If calling `client.swapRestore` or
`client.swapRestoreIndex` directly with that xpub, explicitly pass `"m"` as the
second argument; the maker's root-xpub default otherwise searches different
keys and can return no swaps. Keep the wallet's saved highest allocated index:
these discovery calls are bounded by the gap limit and are not a guarantee that
an empty result makes previously used indices safe to reuse.

```ts
const restoredSwaps = await master.restore(client);
const restoredIndex = await master.restoreIndex(client);
```

Save `swapAuth` from the create response with the swap receipt. KaleidoSwap
cooperative refunds require it in `TxParams`:

```ts
await script.constructRefund({ ...params, swapAuth: receipt.swapAuth });
```

After seed-based recovery, `swapRestore` discovers records but does not return
credentials to an xpub holder. Recover each credential with a restored taker
key, then save it in the receipt:

```ts
// Submarine example: restore keyIndex is a u32 (JS number), while derivation
// accepts a u64 (JS bigint).
const keys = master.deriveSwapKey(BigInt(restored.refundDetails.keyIndex));
const swapAuth = await client.recoverSwapAuth(restored.id, keys.secretKey);
await script.constructRefund({ ...params, swapAuth });
// For a restored chain swap, the same credential authorizes acceptQuote:
await client.acceptQuote(restored.id, quote.amount, swapAuth);
```

Use the refund key for submarine swaps, the claim key for reverse swaps, or
either taker key for chain swaps. This requires maker's signed recovery
endpoints; upstream Boltz does not implement them and needs no `swapAuth`.
