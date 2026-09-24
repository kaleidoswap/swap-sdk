/** Maker pay-through: a Lightning hold invoice funds a direct address payout.
 * The address leg is operator-trusted and has no hash lock. */

export interface PayThroughRequest {
  destination: string;
  /** Satoshis paid on Lightning. Exactly one amount is required. */
  invoiceAmount?: number;
  /** Smallest units received at the destination. Exactly one amount is required. */
  payoutAmount?: number;
  /** Name the destination asset when requesting a conversion, e.g. L-USDT. */
  asset?: string;
  pairHash?: string;
  webhook?: { url: string; [key: string]: unknown };
}

export interface PayThroughCreated {
  id: string;
  /** Keep this credential private; it authorizes actions on this swap. */
  swapAuth: string;
  invoice: string;
  paymentHash: string;
  destination: string;
  destinationLayer: string;
  payoutAsset: string;
  pairId: string;
  invoiceAmount: number;
  payoutAmount: number;
  /** In the payout asset's smallest unit. */
  fees: { protocol: number; network: number; swap: number };
  /** Unix seconds. */
  expiresAt: number;
}

export interface PayThroughStatus {
  id: string;
  type: "reverse";
  status: string;
  paymentStatus: string;
  failureReason: string | null;
  failureDetails: string | null;
  events: Array<{ ts: number; kind: string }>;
  payout: {
    mode: "direct";
    destination: string;
    layer: string | null;
    /** Destination-layer transaction ID once broadcast. */
    reference: string | null;
  };
}

/** The maker answered with an error. A 4xx means nothing was created; a 5xx
 * or gateway error leaves the outcome of a create unknown. */
export class PayThroughApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    public readonly details: string | null = null,
    /** Seconds, from a 429's Retry-After header. */
    public readonly retryAfter: number | null = null,
  ) {
    super(
      `Maker pay-through request failed (${status}: ${code})` +
        (details ? `: ${details}` : ""),
    );
    this.name = "PayThroughApiError";
  }
}

export interface PayThroughClientOptions {
  /** Maker URL ending in /v2. */
  makerUrl: string;
  /** Optional organization attribution key, for server runtimes only. */
  apiKey?: string;
  /** Defaults to global fetch. Useful for controlled hosts and tests. */
  fetch?: typeof fetch;
  /** Per-request timeout. Defaults to 30 seconds. */
  timeoutMs?: number;
}

export interface PayThroughCallOptions {
  signal?: AbortSignal;
}

function object(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Invalid maker pay-through response");
  }
  return value as Record<string, unknown>;
}

function stringField(value: unknown, field: string): string {
  if (typeof value !== "string" || !value) {
    throw new Error(`Invalid maker pay-through response: ${field}`);
  }
  return value;
}

function amount(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Invalid maker pay-through amount: ${field}`);
  }
  return value;
}

function requestedAmount(value: number | undefined, field: string): void {
  if (value !== undefined && (!Number.isSafeInteger(value) || value <= 0)) {
    throw new TypeError(`${field} must be a positive safe integer`);
  }
}

function mismatch(field: string, got: unknown, expected: unknown): Error {
  return new Error(
    `Maker pay-through terms do not match the request: ${field} is ${String(got)}, expected ${String(expected)}`,
  );
}

/** Server runtimes only: an organization key in a browser, worker or mobile
 * bundle is readable by anyone who has the bundle. */
function isServerRuntime(): boolean {
  const g = globalThis as {
    process?: { versions?: { node?: string } };
    Deno?: unknown;
    Bun?: unknown;
  };
  return Boolean(g.process?.versions?.node || g.Deno || g.Bun);
}

const BECH32 = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

function bech32Polymod(values: number[]): number {
  const generators = [
    0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3,
  ];
  let checksum = 1;
  for (const value of values) {
    const top = checksum >>> 25;
    checksum = ((checksum & 0x1ffffff) << 5) ^ value;
    for (let i = 0; i < 5; i++) {
      if ((top >>> i) & 1) checksum ^= generators[i];
    }
  }
  return checksum;
}

/** The amount (millisatoshis) and payment hash of a BOLT11 invoice. The
 * signature is not checked: the maker is who we are comparing it against. */
export function decodeBolt11(invoice: string): {
  amountMsat: bigint | null;
  paymentHash: string;
} {
  const text = invoice.trim().toLowerCase();
  const separator = text.lastIndexOf("1");
  const hrp = text.slice(0, separator);
  if (separator < 1 || !hrp.startsWith("ln")) {
    throw new Error("Invalid BOLT11 invoice");
  }
  const words = Array.from(text.slice(separator + 1), (char) => {
    const word = BECH32.indexOf(char);
    if (word < 0) throw new Error("Invalid BOLT11 invoice");
    return word;
  });
  const expanded = [
    ...Array.from(hrp, (char) => char.charCodeAt(0) >> 5),
    0,
    ...Array.from(hrp, (char) => char.charCodeAt(0) & 31),
  ];
  if (bech32Polymod([...expanded, ...words]) !== 1) {
    throw new Error("Invalid BOLT11 invoice checksum");
  }

  // ln + currency prefix + optional amount: digits and a multiplier.
  const amountMatch = /^ln[a-z]+?(\d+)([munp]?)$/.exec(hrp);
  let amountMsat: bigint | null = null;
  if (amountMatch) {
    const value = BigInt(amountMatch[1]);
    const multiplier = amountMatch[2];
    // 1 BTC = 10^11 msat.
    if (multiplier === "p") {
      if (value % 10n !== 0n) throw new Error("Invalid BOLT11 amount");
      amountMsat = value / 10n;
    } else {
      const scale = {
        "": 100_000_000_000n,
        m: 100_000_000n,
        u: 100_000n,
        n: 100n,
      }[multiplier];
      amountMsat = value * (scale as bigint);
    }
  }

  // 7-word timestamp, then tagged fields, then a 104-word signature and the
  // 6-word checksum.
  const fields = words.slice(7, words.length - 104 - 6);
  let paymentHash: string | null = null;
  for (let i = 0; i + 3 <= fields.length;) {
    const tag = fields[i];
    const length = fields[i + 1] * 32 + fields[i + 2];
    const data = fields.slice(i + 3, i + 3 + length);
    if (data.length !== length) throw new Error("Invalid BOLT11 invoice");
    if (tag === 1 && length === 52) {
      let bits = 0;
      let accumulator = 0;
      let hex = "";
      for (const word of data) {
        accumulator = (accumulator << 5) | word;
        bits += 5;
        while (bits >= 8) {
          bits -= 8;
          hex += ((accumulator >> bits) & 0xff).toString(16).padStart(2, "0");
        }
        accumulator &= (1 << bits) - 1;
      }
      paymentHash = hex.slice(0, 64);
    }
    i += 3 + length;
  }
  if (paymentHash === null)
    throw new Error("BOLT11 invoice has no payment hash");
  return { amountMsat, paymentHash };
}

/** HTTP-only pay-through client; no WASM initialization is needed. */
export class PayThroughClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly request: typeof fetch;
  private readonly timeoutMs: number;

  constructor(options: PayThroughClientOptions) {
    if (options.apiKey && !isServerRuntime()) {
      throw new TypeError(
        "Organization API keys can only be used on a server runtime",
      );
    }
    const url = new URL(options.makerUrl);
    const local = ["localhost", "127.0.0.1", "::1"].includes(url.hostname);
    if (
      (url.protocol !== "https:" && !(local && url.protocol === "http:")) ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      !url.pathname.replace(/\/$/, "").endsWith("/v2")
    ) {
      throw new TypeError(
        "makerUrl must be an HTTPS /v2 URL (HTTP allowed on loopback)",
      );
    }
    this.baseUrl = url.href.replace(/\/$/, "");
    this.apiKey = options.apiKey;
    this.request = options.fetch ?? fetch;
    this.timeoutMs = options.timeoutMs ?? 30_000;
  }

  /** Create the hold invoice and check the maker's terms against the request
   * and the invoice itself. A transport failure or 5xx may leave creation
   * unknown; do not blindly repeat the call or pay a second invoice. */
  async create(
    input: PayThroughRequest,
    options: PayThroughCallOptions = {},
  ): Promise<PayThroughCreated> {
    const destination = input.destination?.trim();
    if (!destination) throw new TypeError("destination is required");
    requestedAmount(input.invoiceAmount, "invoiceAmount");
    requestedAmount(input.payoutAmount, "payoutAmount");
    if (
      (input.invoiceAmount === undefined) ===
      (input.payoutAmount === undefined)
    ) {
      throw new TypeError(
        "Provide exactly one of invoiceAmount or payoutAmount",
      );
    }
    const body: Record<string, unknown> = { destination };
    for (const key of [
      "invoiceAmount",
      "payoutAmount",
      "asset",
      "pairHash",
      "webhook",
    ] as const) {
      if (input[key] !== undefined) body[key] = input[key];
    }
    const raw = object(
      await this.call("swap/pay", {
        method: "POST",
        body: JSON.stringify(body),
        signal: options.signal,
      }),
    );
    const fees = object(raw.fees);
    const created: PayThroughCreated = {
      id: stringField(raw.id, "id"),
      swapAuth: stringField(raw.swapAuth, "swapAuth"),
      invoice: stringField(raw.invoice, "invoice"),
      paymentHash: stringField(raw.paymentHash, "paymentHash"),
      destination: stringField(raw.destination, "destination"),
      destinationLayer: stringField(raw.destinationLayer, "destinationLayer"),
      payoutAsset: stringField(raw.payoutAsset, "payoutAsset"),
      pairId: stringField(raw.pairId, "pairId"),
      invoiceAmount: amount(raw.invoiceAmount, "invoiceAmount"),
      payoutAmount: amount(raw.payoutAmount, "payoutAmount"),
      fees: {
        protocol: amount(fees.protocol, "fees.protocol"),
        network: amount(fees.network, "fees.network"),
        swap: amount(fees.swap, "fees.swap"),
      },
      expiresAt: amount(raw.expiresAt, "expiresAt"),
    };

    if (created.destination !== destination) {
      throw mismatch("destination", created.destination, destination);
    }
    if (
      input.invoiceAmount !== undefined &&
      created.invoiceAmount !== input.invoiceAmount
    ) {
      throw mismatch(
        "invoiceAmount",
        created.invoiceAmount,
        input.invoiceAmount,
      );
    }
    if (
      input.payoutAmount !== undefined &&
      created.payoutAmount < input.payoutAmount
    ) {
      throw mismatch(
        "payoutAmount",
        created.payoutAmount,
        `at least ${input.payoutAmount}`,
      );
    }
    if (
      input.asset !== undefined &&
      created.payoutAsset.toLowerCase() !== input.asset.toLowerCase()
    ) {
      throw mismatch("payoutAsset", created.payoutAsset, input.asset);
    }
    const invoice = decodeBolt11(created.invoice);
    if (invoice.amountMsat !== BigInt(created.invoiceAmount) * 1000n) {
      throw mismatch(
        "invoice amount (msat)",
        invoice.amountMsat,
        BigInt(created.invoiceAmount) * 1000n,
      );
    }
    if (invoice.paymentHash !== created.paymentHash.toLowerCase()) {
      throw mismatch(
        "invoice payment hash",
        invoice.paymentHash,
        created.paymentHash,
      );
    }
    return created;
  }

  /** Poll by swap ID. The maker reports transaction.mempool after broadcast
   * and invoice.settled after settling the Lightning hold. */
  async status(
    id: string,
    options: PayThroughCallOptions = {},
  ): Promise<PayThroughStatus> {
    if (!id || !/^[a-zA-Z0-9_-]+$/.test(id))
      throw new TypeError("Invalid swap id");
    const raw = object(
      await this.call(`swap/${id}`, { method: "GET", signal: options.signal }),
    );
    if (raw.type !== "reverse" || raw.payout == null) {
      throw new Error("Swap is not a pay-through swap");
    }
    const payout = object(raw.payout);
    if (payout.mode !== "direct")
      throw new Error("Swap is not a pay-through swap");
    // Swap IDs are ULIDs: case-insensitive, echoed upper case.
    if (typeof raw.id !== "string" || raw.id.toUpperCase() !== id.toUpperCase())
      throw new Error("Maker pay-through status ID mismatch");
    if (!Array.isArray(raw.events))
      throw new Error("Invalid maker pay-through response: events");
    return {
      id: stringField(raw.id, "id"),
      type: "reverse",
      status: stringField(raw.status, "status"),
      paymentStatus: stringField(raw.paymentStatus, "paymentStatus"),
      failureReason:
        raw.failureReason == null
          ? null
          : stringField(raw.failureReason, "failureReason"),
      failureDetails:
        raw.failureDetails == null
          ? null
          : stringField(raw.failureDetails, "failureDetails"),
      events: raw.events.map((event: unknown) => {
        const entry = object(event);
        return {
          ts: amount(entry.ts, "events.ts"),
          kind: stringField(entry.kind, "events.kind"),
        };
      }),
      payout: {
        mode: "direct",
        destination: stringField(payout.destination, "payout.destination"),
        layer:
          payout.layer == null
            ? null
            : stringField(payout.layer, "payout.layer"),
        reference:
          payout.reference == null
            ? null
            : stringField(payout.reference, "payout.reference"),
      },
    };
  }

  private async call(
    path: string,
    options: { method: "GET" | "POST"; body?: string; signal?: AbortSignal },
  ): Promise<unknown> {
    const headers: Record<string, string> = { Accept: "application/json" };
    if (options.body) headers["Content-Type"] = "application/json";
    if (this.apiKey) headers.Authorization = `Bearer ${this.apiKey}`;

    const controller = new AbortController();
    const timer = setTimeout(
      () => controller.abort(new Error("Maker pay-through request timed out")),
      this.timeoutMs,
    );
    const forward = () => controller.abort(options.signal?.reason);
    if (options.signal?.aborted) forward();
    options.signal?.addEventListener("abort", forward, { once: true });
    try {
      const response = await this.request(`${this.baseUrl}/${path}`, {
        method: options.method,
        body: options.body,
        headers,
        redirect: "error",
        signal: controller.signal,
      });
      const text = await response.text();
      if (!response.ok) {
        let error: Record<string, unknown> = {};
        try {
          const parsed: unknown = JSON.parse(text);
          if (parsed !== null && typeof parsed === "object") {
            error = parsed as Record<string, unknown>;
          }
        } catch {
          // A gateway error page, not a maker error body.
        }
        const retryAfter = Number(response.headers.get("retry-after"));
        throw new PayThroughApiError(
          response.status,
          typeof error.error === "string" ? error.error : "http_error",
          typeof error.details === "string" ? error.details : null,
          Number.isFinite(retryAfter) && retryAfter > 0 ? retryAfter : null,
        );
      }
      try {
        return JSON.parse(text);
      } catch {
        throw new Error("Invalid maker pay-through response: not JSON");
      }
    } finally {
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", forward);
    }
  }
}
