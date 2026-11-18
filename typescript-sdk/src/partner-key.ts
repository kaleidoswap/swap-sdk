// KaleidoSwap partner keys for the HTTP-only entry points.
//
// A partner's `kld_<env>_<key_id>_<secret>` key is a public partner
// identifier: it attributes swaps and carries the partner's fee, and never a
// lower price or higher limits, so it ships inside any app as a build
// constant. The WASM client parses it in Rust; this file holds what the
// HTTP-only clients need without loading WASM.

/**
 * What the maker answered in `X-Kaleido-Attribution`, on a price read or a
 * create:
 *
 * - `"api_key"`: the partner key's price, or a swap stored for that partner;
 * - `"none"`: the public price, or a swap stored without a partner. For a
 *   client with a key, the maker did not recognise it (unknown, revoked, or
 *   for the other environment). That is not an error, so an app with a
 *   rotated key keeps swapping; read this to notice it;
 * - `"org_key"`: a swap stored for an organization key (deprecated).
 *
 * Any other value a newer maker sends is passed through as is.
 */
export type MakerAttribution = "none" | "api_key" | "org_key" | (string & {});

/**
 * Read `X-Kaleido-Attribution` from a response's headers, or `null` when it is
 * absent: a maker that predates it, or a browser whose maker edge does not
 * expose it.
 */
export function readAttribution(headers: {
  get(name: string): string | null;
}): MakerAttribution | null {
  const value = headers.get("X-Kaleido-Attribution");
  return value === null ? null : value.trim();
}

/**
 * Whether a partner key must stay on a server. `false` for every key Platform
 * issues today: they are public partner identifiers, meant to ship inside
 * browser bundles, extensions and mobile apps. Kept for a future secret,
 * server-only key kind, which the clients will refuse outside a server.
 */
export function partnerKeyRequiresServer(apiKey: string): boolean {
  // The `kld_` format carries no kind yet: every key is public.
  void apiKey;
  return false;
}
