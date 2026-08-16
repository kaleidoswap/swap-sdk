# Live corridor probe — `arkade:BTC -> lightning:BTC`

Drives the deployed KaleidoSwap maker on signet with **Ark Labs' own client**
(`@arkade-os/swap`), over both transports.

## Stage 1 — quote only, no funds

```sh
npm i --no-save @arkade-os/sdk @arkade-os/swap light-bolt11-decoder nostr-tools ws
ARKADE_SEED="twelve words" INVOICE="lntbs..." node probe.mjs both
```

The seed needs no funds: `requestLightningSend` derives the lockup and checks
it against the maker's quote **before** any funding. That check is the whole
interop claim — if the two addresses disagreed, a funded swap would die at
`AddressMismatch` and never reach a spend. Getting a verified quote therefore
proves the wire, the quote fields and the derivation, for zero sats.

The invoice must be mutinynet and payable by the maker's node: in this
direction **the maker pays**, so the taker is the one receiving.

`both` runs HTTP and Nostr. Nostr proves something extra — that the published
kind-38859 card is usable, since the client finds the solver by its discovery
key with no base URL known in advance.

## Stage 2 — funded

Fund the Arkade wallet, then `wallet.send({address, amount: fundAmount})` from
the returned quote. This is the only way to exercise a **v2 claim leaf against
real arkd**, which stage 1 deliberately cannot cover.

Only worth paying for once stage 1 is green.

## Overrides

`MAKER_URL`, `ARK_SERVER_URL`, `RELAY_URL`, `SOLVER_PUBKEY` — defaults point at
signet, mutinynet arkd, our relay, and our published discovery key.
