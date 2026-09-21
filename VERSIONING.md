# Versioning and support

What this SDK promises a consumer about change.
[`docs/releasing.md`](docs/releasing.md) covers how a release is built and
published; this covers what you can rely on between them.

## Today: `0.x`

**The public surface can change in any minor release.** Between `0.1.0` and
`0.8.0` — six and a half weeks — it changed eight times. That is what a
`0.x` version means and we are not going to pretend otherwise.

Pin an exact version. A caret range on a `0.x` package resolves across minors,
and our minors move.

```json
"@kaleidorg/swap-sdk": "0.8.0"
```

What already holds:

- **One version across every surface.** The Rust crate, the PyPI distribution
  and the npm package share one `X.Y.Z`. A version that exists on one registry
  and not another is a release failure, not a variant.
- **Patches never break.** `0.8.1` is a fix to `0.8.0` and nothing else.
- **Breaking changes land in a minor** — `0.9.0`, never `0.8.1`.
- **Every release is in [`CHANGELOG.md`](CHANGELOG.md)** before it is published.

What does not hold yet, and the gap this document closes:

- Breaking changes are described in the changelog but **not labelled**. Eight
  minors carry zero `BREAKING` markers, so finding what will break you means
  reading prose under `### Changed — <topic>` headings.
- **Nothing is deprecated before it is removed.** A symbol can be present in
  `0.8.0` and gone in `0.9.0` with no intermediate release that warns.
- **No support window.** Only the newest version has ever received a fix.

## From the next minor

Three changes, none of which slow a release down:

1. **Breaking changes are labelled.** A `### BREAKING` block at the top of the
   release's changelog entry, one line per change, each naming the old symbol,
   the new one, and the edit that migrates. If it is not in that block it is not
   a breaking change, and a consumer can skim one heading instead of a release.
2. **Removal takes two minors.** A symbol slated for removal is marked
   `@deprecated` in the types with its replacement, kept working, and removed no
   earlier than the minor after that. A consumer on any release can reach the
   next one without a cliff.
3. **Fixes land on the current and previous minor.** Security fixes go to both;
   other fixes to the current minor, and to the previous one where the backport
   is cheap.

## `1.0`

`1.0` is not a maturity badge; it is the point at which the promises above get
stricter, so it is gated on things that would otherwise force a break:

- **mainnet is live** and `forNetwork("mainnet")` resolves. Until then the SDK
  cannot serve real money, and a surface frozen before it has served any is
  frozen on a guess.
- **The Boltz-derived names are gone** from the public surface. `BoltzClient`
  is the fork's vocabulary, and renaming it after `1.0` costs a major.
- **No planned surface changes** in the next two minors.

From `1.0`, semver as written: breaking changes only in a major, and a
deprecated symbol survives **six months or two minors, whichever is longer**.

## Networks

Network support is part of the contract: a network the SDK will not resolve is
a network you cannot ship on.

| Network | Status |
|---|---|
| `signet` | Supported. The live KaleidoSwap maker, settling on Mutinynet. |
| `regtest` | Supported, against this repository's harness. |
| `mainnet` | **Rejected** by `forNetwork` — no mainnet maker yet. |
| `testnet` | Rejected. A chain identity only — we run no testnet3 maker. |

`mainnet` is rejected rather than silently resolved to a third-party maker.
When it lands, it lands as a minor with a changelog entry, not a quiet flip.

## Reporting

Breaking changes we failed to label, or a removal without its two minors, are
bugs — open an issue. Security reports go to security@kaleidoswap.com.
