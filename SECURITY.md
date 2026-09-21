# Security Policy

`kaleidorg-swap-sdk` builds, validates and signs the transactions that move
funds through a swap. We take security seriously and welcome responsible
disclosure.

> **Signet only.** `forNetwork` rejects `mainnet` — there is no mainnet maker
> yet. Nothing here has moved real money, and nothing here has been audited.

## Reporting a Vulnerability

Please **DO NOT** open a public issue for security vulnerabilities. Instead,
email:

**security@kaleidoswap.com**

### What to Include

- A description of the vulnerability
- Steps to reproduce
- Potential impact (e.g. fund loss, unrefundable lockup, key or preimage
  exposure)
- Any suggested fix
- Your contact information for follow-up

### Response Timeline

- **Initial response**: within 48 hours
- **Status updates**: every 7 days until resolved
- **Fix**: as quickly as possible, typically within 30 days

### Disclosure Policy

- Please give us reasonable time to investigate and fix before going public.
- We credit researchers who report responsibly (unless you prefer anonymity).
- Once a fix ships, we publish an advisory describing the issue and the fix.

## Threat Model Notes

When auditing this SDK, the highest-impact surfaces are:

- **Response validation before funding.** `SwapScript::submarine_from_swap_resp`
  and the reverse and chain equivalents re-derive the lockup from the maker's
  create response against the caller's own key. This is the check that stands
  between a caller and a lockup only the maker can spend, and it has to run
  before funding — a bypass, or a validation that accepts a tree the caller
  cannot refund, is the worst bug this SDK can have.
- **Swap key and preimage derivation.** Keys and preimages derive from a BIP85
  child (index 26589) of a wallet mnemonic. Reusing a swap index reuses the
  claim and refund material, and on a reverse swap the preimage is the secret
  the payment buys.
- **`swapAuth`.** A per-swap credential, issued exactly once on create, that
  authorizes accepting a chain-swap re-quote — the taker's full capability over
  that swap. It is a plain property of the create response, so anything that
  logs the response discloses it, and nothing re-issues a lost one.
- **Partner API keys.** `kld_test_…` / `kld_live_…` organization keys travel
  as bearer credentials bound to the maker URL they were configured with, are
  refused over plain HTTP off loopback, decline redirects on the native
  clients, and are refused outright in a document context. The browser is the
  weak case: `fetch` owns redirects, so a hop off the maker is reported after
  the fact rather than declined.
- **Chain identity.** Signet settles on Mutinynet, and signet and testnet3
  encode addresses identically — a mismatch raises no error, it simply creates
  swaps on one chain while funding or watching another. The same shape of bug
  exists across Liquid networks.
- **Caller-funded Liquid PSETs.** `prepare_liquid_claim` and
  `prepare_liquid_refund` hand a template to a wallet and verify what comes
  back. The verification
  rejects asset substitution, payout skimming, excess fees, duplicate inputs,
  changed prevouts and confidentiality downgrades; weaknesses in it are
  directly exploitable by the funding wallet.
- **Cooperative paths.** Cooperative claims and refunds are MuSig2 keyspends
  co-signed with the maker. A cooperative refund spends with no locktime, so
  the signing session is what stands in for the timeout.

## Supported Versions

See [`VERSIONING.md`](VERSIONING.md).
