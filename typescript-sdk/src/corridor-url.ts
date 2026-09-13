/**
 * Where the Arkade Intents corridor lives relative to a maker's `/v2` base.
 *
 * The corridor is a **sibling** of the Boltz-shaped routes, not a child:
 * `POST /v1/swap` and `GET /v1/rfq/{id}` hang off the maker's origin. So a
 * client configured with `https://maker/v2` reaches the corridor at
 * `https://maker/v1/...`, and this is the one rule that derives the former
 * from the latter.
 *
 * It lives in its own module, with no wasm import, so both package entries
 * can share it: the main entry mirrors it against the Rust
 * `corridor_root_from_maker_url` (the wasm `corridorUrl` getter), and the
 * `./arkade` venue uses it to build an `httpTransport` from the same
 * `makerUrl` the rest of the SDK is configured with — without pulling the
 * wasm binary into a bundle that only wants the venue.
 *
 * The `/v2` suffix is **required**, not stripped when present: a URL without
 * it is not a maker base this SDK recognises, and guessing an origin would
 * send a request — and any organization API key riding on it — somewhere the
 * caller did not name. A trailing slash is tolerated; a query or fragment is
 * not, since neither belongs on a base URL.
 */
export function corridorRootFromMakerUrl(makerUrl: string): string {
  const url = new URL(makerUrl);
  if (url.search !== "" || url.hash !== "") {
    throw new Error(
      `maker URL ${makerUrl} carries a query or fragment, which a base URL cannot`,
    );
  }
  const path = url.pathname.replace(/\/+$/, "");
  if (!path.endsWith("/v2")) {
    throw new Error(
      `maker URL ${makerUrl} does not end in /v2 — the Intents corridor is a ` +
        "sibling of the /v2 routes, so its origin can only be derived from a /v2 base",
    );
  }
  url.pathname = path.slice(0, -"/v2".length);
  return url.toString().replace(/\/+$/, "");
}
