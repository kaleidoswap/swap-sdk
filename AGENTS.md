# AGENTS.md

Instructions for coding agents (Claude Code, Codex, Cursor, …) working in this
repository. Humans want the [README](README.md), [SECURITY.md](SECURITY.md) and
[docs/releasing.md](docs/releasing.md).

## This repository is public: fix security bugs without disclosing them

Everything pushed here, including pull-request titles and bodies, commit
messages, issues, comments and `CHANGELOG.md`, is visible to anyone the moment
it lands. The published packages on npm, PyPI and the GitHub releases lag
`trunk`, so a merged fix is not a shipped fix. Until a release that contains
the fix is published, a public description of the bug is a public exploit
guide for every current user.

Rules, in order of precedence:

1. **Check visibility before any outward-facing action**
   (`gh repo view --json visibility`). Assume public.
2. **Never publish, anywhere public, before the fixed release exists:** a
   reference to an unpublished security review or audit, finding numbers or
   severity ratings, exploit or attack scenarios, "a malicious maker can …"
   narratives, or anything else that explains how to abuse an unpatched
   version. This includes PR bodies, commit messages, code comments, tests
   named after the attack, and `CHANGELOG.md` under `[Unreleased]`.
3. **Describe the change neutrally.** "Validate the invoice amount against
   the request" is fine. "Fixes finding 4: the maker could return a 10x
   invoice and an automated payer overpays" is not, until the advisory.
4. **Prefer the private path for anything exploitable:** a GitHub security
   advisory with a temporary private fork (Security tab → Advisories → New
   draft → "Start a temporary private fork"), or at least a branch the
   maintainer agrees to merge and release promptly. Release first, then the
   maintainer publishes the advisory with the full write-up and credit.
5. **When in doubt, stop and ask the maintainer** before pushing or opening
   anything. A delay costs hours; a disclosure cannot be taken back, because
   GitHub keeps PR and comment edit history.
6. **Pass these rules on explicitly** when delegating to subagents or other
   tools: they do not inherit them.

What *is* fine to write publicly: the neutral code change, its tests, a
changelog line that says what the SDK now checks, and a pointer to the
advisory once it exists.

## Working here

- Rust core in `src/`, UniFFI bindings (Python, React Native) in `bindings/`,
  WASM in `bindings-wasm/`, the npm package in `typescript-sdk/`. A safety
  check belongs in the Rust core so every binding inherits it.
- Release process: `docs/releasing.md`. Versions are synchronized with
  `make sync-version VERSION=X.Y.Z` and checked with `make validate-versions`.
- Every change gets a `CHANGELOG.md` entry under `[Unreleased]` in the
  existing style.
- Run `cargo fmt --all`, `make cargo-clippy`, the relevant `cargo test`
  targets, `make check-binding-parity` when the WASM surface changes, and
  `make generate-python-bindings` when the UniFFI interface changes. Never
  share one `CARGO_TARGET_DIR` between worktrees: cargo overwrites metadata
  across them and serves stale test binaries.
