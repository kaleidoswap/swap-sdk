// KaleidoSwap partner keys for the HTTP-only entry points.
//
// A partner's `kld_<env>_<key_id>_<secret>` key is a public partner
// identifier: it attributes swaps and carries the partner's fee, and never a
// lower price or higher limits, so it ships inside any app as a build
// constant. The WASM client parses it in Rust; this file holds what the
// HTTP-only clients need without loading WASM.

/**
 * The header the maker answers swap creations and price reads with: `none` or
 * `api_key`. On a create it names the attribution the swap was stored with; on
 * a price read, whose price it answered with. A browser can read it only where
 * the maker's edge exposes it. See {@link MakerAttribution}.
 */
export const ATTRIBUTION_HEADER = "X-Kaleido-Attribution";

/**
 * What the maker answered in `X-Kaleido-Attribution`, on a price read or a
 * create:
 *
 * - `"api_key"`: the partner key's price, or a swap stored for that partner;
 * - `"none"`: the public price, or a swap stored without a partner. For a
 *   client with a key, the maker did not recognise it (unknown, revoked, or
 *   for the other environment). That is not an error, so an app with a
 *   rotated key keeps swapping; read this to notice it;
 *
 * Any other value a newer maker sends is passed through as is.
 */
export type MakerAttribution = "none" | "api_key" | (string & {});

/**
 * Read `X-Kaleido-Attribution` from a response's headers, or `null` when it is
 * absent: a maker that predates it, or a browser whose maker edge does not
 * expose it.
 */
export function readAttribution(headers: {
  get(name: string): string | null;
}): MakerAttribution | null {
  const value = headers.get(ATTRIBUTION_HEADER);
  return value === null ? null : value.trim();
}

/** Parse the public partner key accepted by the Rust client. */
export function normalizePartnerKey(apiKey: string): string {
  // Rust trims surrounding ASCII whitespace before parsing the header value.
  const key =
    typeof apiKey === "string"
      ? apiKey.replace(/^[\t\n\f\r ]+|[\t\n\f\r ]+$/g, "")
      : "";
  if (
    key.length > 512 ||
    !/^kld_(?:test|live)_[A-Za-z0-9-]+_[!-~]+$/.test(key)
  ) {
    throw new TypeError(
      "apiKey must be a valid kld_test_ or kld_live_ partner key",
    );
  }
  return key;
}

/** Every currently supported partner key is public. Unknown formats are refused. */
export function partnerKeyRequiresServer(apiKey: string): boolean {
  normalizePartnerKey(apiKey);
  return false;
}
