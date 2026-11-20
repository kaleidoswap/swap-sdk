/**
 * The in-app check for `SwapClient.kaleidoMaker`.
 *
 * `kaleidoMaker` takes a partner's `kld_test_…` / `kld_live_…` key. Every key
 * Platform issues today is a public partner identifier: it attributes swaps
 * and carries the partner's fee, never a lower price or higher limits, so it
 * belongs in the app as a build constant and `kaleidoMaker` builds without any
 * opt-in.
 *
 * The refusal below is kept for a future secret, server-only key kind. The
 * `bindings` crate also builds the Python wheel, where a server holding such a
 * key is the intended use, so the Rust side has no way to know it is inside an
 * app; this package only ever runs inside one. So the check lives here, in
 * this package's TypeScript, with no runtime sniffing to get wrong.
 *
 * `scripts/guard-kaleido-maker.mjs` wires this into the generated
 * `SwapClient.kaleidoMaker` after every `ubrn` generation, and refuses to finish
 * if it cannot find that method. Editing the generated file by hand would be
 * lost on the next build.
 *
 * Erasable TypeScript only: the package tests import this file straight into
 * Node, which strips the types and runs what is left.
 */

/** The fourth argument to `SwapClient.kaleidoMaker`. */
export interface KaleidoMakerAppOptions {
  /**
   * Build the client inside the app with a secret, server-only key. Has no
   * effect for the keys Platform issues today, which are public and need no
   * opt-in. Only the boolean `true` counts.
   */
  allowMobile?: boolean;
}

/** What a partner key may be shipped in: `"public"` for every key today. */
export type PartnerKeyKind = "public" | "secret";

/** Why `kaleidoMaker` refused a secret key. Never includes the key. */
export const KALEIDO_MAKER_IN_APP_REFUSAL =
  "refusing to build a client with a secret partner key in a mobile app: a " +
  "key in an app bundle is readable by anyone who installs it. Create those " +
  "swaps from your server with the key in server-side configuration, or pass " +
  "{ allowMobile: true } as the fourth argument if you have accepted that " +
  "exposure. Public kld_ keys need no opt-in.";

/**
 * The refusal message, or `undefined` when the client may be built: always for
 * a public key, and for a secret one only when the caller opted in.
 *
 * It returns the message rather than throwing it so that the generated code
 * can throw its own `Exception.Generic`, the same error type every other
 * failure of this constructor has.
 */
export function kaleidoMakerRefusal(
  options: KaleidoMakerAppOptions | undefined,
  keyKind: PartnerKeyKind = "public",
): string | undefined {
  if (keyKind === "public" || options?.allowMobile === true) {
    return undefined;
  }
  return KALEIDO_MAKER_IN_APP_REFUSAL;
}
