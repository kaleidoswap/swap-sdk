/** Confirmation target (blocks) -> fee rate in sat/vB. Missing targets are
 * undefined; callers must select an available target explicitly. */
export type EsploraFeeEstimates = Readonly<Partial<Record<number, number>>>;

export interface EsploraDataOptions {
  /** Per-call deadline, including reading the body. Defaults to 30 seconds. */
  timeoutMs?: number;
  signal?: AbortSignal;
  /** Defaults to global fetch. No maker credentials are sent. */
  fetch?: typeof fetch;
}

/** HTTP failures and malformed successful responses remain distinguishable. */
export class EsploraDataError extends Error {
  constructor(
    public readonly code: "HTTPStatusNotSuccess" | "HTTPResponseBodyInvalid",
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "EsploraDataError";
  }
}

async function esploraText(
  baseUrl: string,
  path: string,
  options: EsploraDataOptions,
): Promise<{ text: string; status: number }> {
  const base = new URL(baseUrl);
  if (
    !["https:", "http:"].includes(base.protocol) ||
    base.username ||
    base.password ||
    base.search ||
    base.hash
  ) {
    throw new TypeError(
      "Esplora base URL must be HTTP(S), without credentials, query or fragment",
    );
  }
  const timeoutMs = options.timeoutMs ?? 30_000;
  if (
    !Number.isSafeInteger(timeoutMs) ||
    timeoutMs <= 0 ||
    timeoutMs > 2_147_483_647
  ) {
    throw new TypeError(
      "timeoutMs must be a positive integer no greater than 2147483647",
    );
  }
  const request = options.fetch ?? globalThis.fetch;
  const controller = new AbortController();
  const forward = () => controller.abort(options.signal?.reason);
  if (options.signal?.aborted) forward();
  controller.signal.throwIfAborted();
  options.signal?.addEventListener("abort", forward, { once: true });
  const timer = setTimeout(
    () =>
      controller.abort(
        new DOMException("Esplora request timed out", "TimeoutError"),
      ),
    timeoutMs,
  );
  try {
    const response = await request(`${base.href.replace(/\/+$/, "")}/${path}`, {
      method: "GET",
      headers: {
        Accept: path === "fee-estimates" ? "application/json" : "text/plain",
      },
      redirect: "error",
      credentials: "omit",
      signal: controller.signal,
    });
    if (!response.ok) {
      await response.body?.cancel();
      throw new EsploraDataError(
        "HTTPStatusNotSuccess",
        `Esplora request failed (${response.status})`,
        response.status,
      );
    }
    return { text: await response.text(), status: response.status };
  } finally {
    clearTimeout(timer);
    options.signal?.removeEventListener("abort", forward);
  }
}

/** Fetch `GET <esplora base>/fee-estimates` for the explicitly chosen chain
 * endpoint. Values are sat/vB, including fractional Liquid rates. There is no
 * automatic retry, default fee, or fallback to the maker's fixed Liquid fee. */
export async function esploraFeeEstimates(
  baseUrl: string,
  options: EsploraDataOptions = {},
): Promise<EsploraFeeEstimates> {
  const { text, status } = await esploraText(baseUrl, "fee-estimates", options);
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new EsploraDataError(
      "HTTPResponseBodyInvalid",
      "Invalid Esplora fee estimates JSON",
      status,
    );
  }
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    throw new EsploraDataError(
      "HTTPResponseBodyInvalid",
      "Esplora fee estimates must be an object",
      status,
    );
  }
  const estimates: Partial<Record<number, number>> = {};
  for (const [target, rate] of Object.entries(raw)) {
    if (
      !/^[1-9][0-9]*$/.test(target) ||
      Number(target) > 0xffff_ffff ||
      typeof rate !== "number" ||
      !Number.isFinite(rate) ||
      rate <= 0
    ) {
      throw new EsploraDataError(
        "HTTPResponseBodyInvalid",
        "Invalid Esplora confirmation target or sat/vB rate",
        status,
      );
    }
    estimates[Number(target)] = rate;
  }
  return Object.freeze(estimates);
}

/** Choose the largest available confirmation target <= the requested target.
 * This uses an equally fast or faster estimate, never a slower-target fallback.
 * No estimate (including an empty map) returns undefined; there is no default.
 * Rates remain in sat/vB, including fractional Liquid values. */
export function selectEsploraFeeRate(
  estimates: EsploraFeeEstimates,
  target: number,
): number | undefined {
  if (!Number.isInteger(target) || target <= 0 || target > 0xffff_ffff) {
    throw new TypeError("target must be a positive uint32 block count");
  }
  let selected = 0;
  let rate: number | undefined;
  for (const [key, value] of Object.entries(estimates)) {
    if (value === undefined) continue;
    const blocks = Number(key);
    if (
      !/^[1-9][0-9]*$/.test(key) ||
      blocks > 0xffff_ffff ||
      !Number.isFinite(value) ||
      value <= 0
    ) {
      throw new TypeError("invalid confirmation target or sat/vB estimate");
    }
    if (blocks <= target && blocks > selected) {
      selected = blocks;
      rate = value;
    }
  }
  return rate;
}

/** Fetch `GET <esplora base>/blocks/tip/height`. An unavailable or malformed
 * tip rejects; it never becomes a zero placeholder. A genuine Esplora genesis
 * tip (0) is valid. Endpoint/network selection belongs to the caller. */
export async function esploraTipHeight(
  baseUrl: string,
  options: EsploraDataOptions = {},
): Promise<number> {
  const response = await esploraText(baseUrl, "blocks/tip/height", options);
  const text = response.text.trim();
  const status = response.status;
  if (!/^(0|[1-9][0-9]*)$/.test(text) || Number(text) > 0xffff_ffff) {
    throw new EsploraDataError(
      "HTTPResponseBodyInvalid",
      "Invalid Esplora tip height",
      status,
    );
  }
  return Number(text);
}
