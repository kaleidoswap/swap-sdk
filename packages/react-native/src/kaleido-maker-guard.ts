/**
 * The in-app refusal for `SwapClient.kaleidoMaker`.
 *
 * `kaleidoMaker` takes the partner organization's secret `kld_test_…` /
 * `kld_live_…` key. The `bindings` crate it comes from also builds the Python
 * wheel, where it runs on a server and holding the key is the point, so the
 * Rust side has no way to know it is inside an app. This package only ever runs
 * inside one: a turbo module in an iOS or Android bundle, where every user
 * holds the binary and can read the key out of it. So the refusal lives here,
 * in this package's TypeScript, with no runtime sniffing to get wrong.
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
   * Build the attributed client inside the app anyway. Off by default, and
   * `kaleidoMaker` throws without it. Set it only if you have accepted that
   * everyone who installs the app can read the organization key out of it and
   * attribute their own swaps to your organization, or exhaust its limits.
   * Only the boolean `true` counts.
   */
  allowMobile?: boolean;
}

/** Why `kaleidoMaker` refused. Never includes the key. */
export const KALEIDO_MAKER_IN_APP_REFUSAL =
  "refusing to build an attributed client in a mobile app: the organization " +
  "API key is a permanent credential with no origin binding and no per-key " +
  "rate limit, so a key in an app bundle is readable by anyone who installs " +
  "it. Create attributed swaps from your server with the key in server-side " +
  "configuration, or pass { allowMobile: true } as the fourth argument if " +
  "you have accepted that exposure.";

/**
 * The refusal message, or `undefined` when the caller opted in.
 *
 * It returns the message rather than throwing it so that the generated code
 * can throw its own `Exception.Generic`, the same error type every other
 * failure of this constructor has.
 */
export function kaleidoMakerRefusal(
  options: KaleidoMakerAppOptions | undefined,
): string | undefined {
  return options?.allowMobile === true ? undefined : KALEIDO_MAKER_IN_APP_REFUSAL;
}
