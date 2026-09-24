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

export class PayThroughApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
  ) {
    super(`Maker pay-through request failed (${status}: ${code})`);
    this.name = "PayThroughApiError";
  }
}

export interface PayThroughClientOptions {
  /** Maker URL ending in /v2. */
  makerUrl: string;
  /** Optional organization attribution key. Never put a private key in browser code. */
  apiKey?: string;
  /** Defaults to global fetch. Useful for controlled hosts and tests. */
  fetch?: typeof fetch;
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

/** HTTP-only pay-through client; no WASM initialization is needed. */
export class PayThroughClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly request: typeof fetch;

  constructor(options: PayThroughClientOptions) {
    if (options.apiKey && typeof document !== "undefined") {
      throw new TypeError("Organization API keys cannot be used in a browser");
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
  }

  /** Create the hold invoice. A transport failure may leave creation unknown;
   * do not blindly repeat the call or pay a second invoice. */
  async create(input: PayThroughRequest): Promise<PayThroughCreated> {
    if (!input.destination?.trim())
      throw new TypeError("destination is required");
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
    const raw = object(
      await this.call("swap/pay", {
        method: "POST",
        body: JSON.stringify(input),
      }),
    );
    const fees = object(raw.fees);
    return {
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
  }

  /** Poll by swap ID. The maker reports transaction.mempool after broadcast
   * and invoice.settled after settling the Lightning hold. */
  async status(id: string): Promise<PayThroughStatus> {
    if (!id || !/^[a-zA-Z0-9_-]+$/.test(id))
      throw new TypeError("Invalid swap id");
    const raw = object(await this.call(`swap/${id}`, { method: "GET" }));
    if (raw.type !== "reverse" || raw.payout == null) {
      throw new Error("Swap is not a pay-through swap");
    }
    const payout = object(raw.payout);
    if (payout.mode !== "direct")
      throw new Error("Swap is not a pay-through swap");
    if (raw.id !== id) throw new Error("Maker pay-through status ID mismatch");
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
    options: { method: "GET" | "POST"; body?: string },
  ): Promise<unknown> {
    const headers: Record<string, string> = { Accept: "application/json" };
    if (options.body) headers["Content-Type"] = "application/json";
    if (this.apiKey) headers.Authorization = `Bearer ${this.apiKey}`;
    const response = await this.request(`${this.baseUrl}/${path}`, {
      method: options.method,
      body: options.body,
      headers,
      redirect: "error",
    });
    const body: unknown = await response.json();
    if (!response.ok) {
      const error =
        body !== null && typeof body === "object"
          ? (body as Record<string, unknown>)
          : {};
      throw new PayThroughApiError(
        response.status,
        typeof error.error === "string" ? error.error : "unknown_error",
      );
    }
    return body;
  }
}
