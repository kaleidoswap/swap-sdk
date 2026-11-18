// KaleidoSwap organization keys for the HTTP-only entry points.
//
// The WASM client parses an org key in Rust, curve check included. The
// pay-through client and the Arkade transport load no WASM, so they check the
// shape here and leave the curve to the maker, which answers an off-curve key
// with `400 org_key_malformed` and never with an anonymous swap.

/** The header an organization key travels in, on swap creation only. */
export const ORG_KEY_HEADER = "X-Kaleido-Org-Key";

/**
 * The header the maker answers every swap creation with: `none`, `api_key` or
 * `org_key`, naming the attribution the swap was stored with. A browser can
 * read it only where the maker's edge exposes it.
 */
export const ATTRIBUTION_HEADER = "X-Kaleido-Attribution";

/**
 * An organization key in the form every KaleidoSwap system stores it: 64
 * lowercase hex. Throws a `TypeError` naming the problem for anything else.
 *
 * The key is public, so the message may quote it; it never quotes a value long
 * enough to be something else pasted into the wrong field, such as a `kld_` key.
 */
export function normalizeOrgKey(value: string): string {
  const key = value.trim();
  if (!/^[0-9a-fA-F]{64}$/.test(key)) {
    const problem =
      key.length === 64
        ? "a character that is not hex"
        : `${key.length} characters`;
    throw new TypeError(
      "orgKey must be the 64-hex x-only public key registered in the partner " +
        `panel (got ${problem})`,
    );
  }
  return key.toLowerCase();
}
