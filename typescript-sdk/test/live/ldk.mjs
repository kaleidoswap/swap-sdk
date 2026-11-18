// A few ldk-server gRPC calls, enough to mint and pay BOLT11 invoices from
// the live suite: HTTP/2 from node:http2, the protobuf messages by hand, and
// ldk-server's `macaroon` header, a request-bound copy of the node's macaroon.
import { createHash, createHmac } from "node:crypto";
import { connect } from "node:http2";

const SERVICE = "/api.LightningNode/";

// Protobuf, the parts these messages use.
function varint(n) {
  let v = BigInt(n);
  const out = [];
  do {
    let byte = Number(v & 0x7fn);
    v >>= 7n;
    if (v) byte |= 0x80;
    out.push(byte);
  } while (v);
  return Buffer.from(out);
}

function field(number, wireType, payload) {
  return Buffer.concat([varint((number << 3) | wireType), payload]);
}

const uint = (number, value) => field(number, 0, varint(value));

function bytes(number, value) {
  const buf = Buffer.isBuffer(value) ? value : Buffer.from(value, "utf8");
  return field(number, 2, Buffer.concat([varint(buf.length), buf]));
}

/** `{ fieldNumber: [values] }`: varints as bigint, the rest as Buffers. */
function decode(buf) {
  const fields = {};
  let at = 0;
  const readVarint = () => {
    let v = 0n;
    let shift = 0n;
    for (;;) {
      const byte = buf[at++];
      v |= BigInt(byte & 0x7f) << shift;
      if (!(byte & 0x80)) return v;
      shift += 7n;
    }
  };
  while (at < buf.length) {
    const key = Number(readVarint());
    const [number, wireType] = [key >> 3, key & 7];
    let value;
    if (wireType === 0) value = readVarint();
    else if (wireType === 2) {
      const len = Number(readVarint());
      value = buf.subarray(at, at + len);
      at += len;
    } else if (wireType === 1) {
      value = buf.subarray(at, at + 8);
      at += 8;
    } else if (wireType === 5) {
      value = buf.subarray(at, at + 4);
      at += 4;
    } else throw new Error(`ldk: unsupported protobuf wire type ${wireType}`);
    (fields[number] ??= []).push(value);
  }
  return fields;
}

// Macaroons, v2 binary format with first-party caveats only, as ldk-server
// reads them. Each request sends a copy of the reusable macaroon with one more
// caveat, `request = <unix seconds> <Method> <sha256(gRPC body)>`, which binds
// it to that method, the exact framed body and the current time (ldk-server's
// docs/request-binding.md). The reusable macaroon itself is never sent.
const REQUEST_CAVEAT_PREFIX = "request = ";
const MAX_MACAROON_BYTES = 4096;

function packet(kind, value) {
  return Buffer.concat([varint(kind), varint(value.length), value]);
}

/** `{ location, identifier, caveats, signature }` from a hex v2 macaroon. */
export function parseMacaroon(hex) {
  const token = hex.trim();
  if (
    !/^(?:[0-9a-fA-F]{2})+$/.test(token) ||
    token.length > MAX_MACAROON_BYTES * 2
  )
    throw new Error("ldk: the macaroon is not a hex-encoded v2 macaroon");
  const buf = Buffer.from(token, "hex");
  let at = 1;
  const read = () => {
    let value = 0;
    for (let shift = 0; ; shift += 7) {
      if (at >= buf.length || shift > 28)
        throw new Error("ldk: truncated or malformed macaroon");
      const byte = buf[at++];
      value += (byte & 0x7f) * 2 ** shift;
      if (!(byte & 0x80)) return value;
    }
  };
  const field = () => {
    const kind = read();
    if (kind === 0) return [0, null];
    const len = read();
    if (at + len > buf.length) throw new Error("ldk: truncated macaroon");
    const value = buf.subarray(at, at + len);
    at += len;
    return [kind, value];
  };
  if (buf[0] !== 2) throw new Error("ldk: only v2 macaroons are supported");
  let location = null;
  let [kind, value] = field();
  if (kind === 1) {
    location = value;
    [kind, value] = field();
  }
  if (kind !== 2 || !value.length || field()[0] !== 0)
    throw new Error("ldk: malformed macaroon identifier");
  const identifier = value;
  const caveats = [];
  for (;;) {
    [kind, value] = field();
    if (kind === 0) break;
    if (kind !== 2 || field()[0] !== 0)
      throw new Error("ldk: only first-party macaroon caveats are supported");
    caveats.push(value);
  }
  [kind, value] = field();
  if (kind !== 6 || value.length !== 32 || at !== buf.length)
    throw new Error("ldk: malformed macaroon signature");
  return { location, identifier, caveats, signature: value };
}

function serializeMacaroon({ location, identifier, caveats, signature }) {
  const zero = Buffer.from([0]);
  return Buffer.concat([
    Buffer.from([2]),
    ...(location ? [packet(1, location)] : []),
    packet(2, identifier),
    zero,
    ...caveats.flatMap((caveat) => [packet(2, caveat), zero]),
    zero,
    packet(6, signature),
  ]);
}

/**
 * The hex macaroon to send with one request: `macaroonHex` plus a caveat that
 * binds it to `method` (the short RPC name), the exact gRPC `body` including
 * its five-byte frame header, and `timestamp` (Unix seconds).
 */
export function bindMacaroonToRequest(
  macaroonHex,
  method,
  body,
  timestamp = Math.floor(Date.now() / 1000),
) {
  if (!/^[A-Za-z0-9]{1,128}$/.test(method))
    throw new Error(`ldk: invalid RPC method name ${JSON.stringify(method)}`);
  const macaroon = parseMacaroon(macaroonHex);
  if (
    macaroon.caveats.some((c) =>
      c.toString("latin1").startsWith(REQUEST_CAVEAT_PREFIX),
    )
  )
    throw new Error("ldk: pass the reusable macaroon, not a request-bound one");
  const digest = createHash("sha256").update(body).digest("hex");
  const caveat = Buffer.from(
    `${REQUEST_CAVEAT_PREFIX}${BigInt(timestamp)} ${method} ${digest}`,
    "latin1",
  );
  const bound = serializeMacaroon({
    ...macaroon,
    caveats: [...macaroon.caveats, caveat],
    signature: createHmac("sha256", macaroon.signature).update(caveat).digest(),
  });
  if (bound.length > MAX_MACAROON_BYTES)
    throw new Error("ldk: the macaroon has no room for a request caveat");
  return bound.toString("hex");
}

const str = (fields, n) => fields[n]?.[0]?.toString("utf8") ?? "";
const num = (fields, n) => fields[n]?.[0] ?? 0n;

export const PAYMENT_STATUS = ["pending", "succeeded", "failed"];

export class LdkServer {
  #macaroon;

  /**
   * @param {string} address `host:port` of the gRPC listener.
   * @param {string} macaroonHex a reusable macaroon, hex-encoded, such as the
   *   node's `<storage dir>/<network>/macaroons/admin.macaroon`.
   * @param {string | Buffer} certPem the node's TLS certificate.
   */
  constructor(address, macaroonHex, certPem) {
    parseMacaroon(macaroonHex);
    this.address = address;
    this.certPem = certPem;
    // Not a public field, so the credential stays out of logs and inspection.
    this.#macaroon = macaroonHex.trim();
  }

  async #call(method, message) {
    const frame = Buffer.alloc(5 + message.length);
    frame.writeUInt32BE(message.length, 1);
    message.copy(frame, 5);
    const macaroon = bindMacaroonToRequest(this.#macaroon, method, frame);

    const session = connect(`https://${this.address}`, {
      ca: this.certPem,
      // The e2e certificates name the container, not the published port.
      checkServerIdentity: () => undefined,
    });
    try {
      return await new Promise((resolve, reject) => {
        session.on("error", reject);
        const stream = session.request({
          ":method": "POST",
          ":path": SERVICE + method,
          "content-type": "application/grpc+proto",
          te: "trailers",
          macaroon,
        });
        const chunks = [];
        let headers = {};
        stream.on("response", (h) => (headers = h));
        stream.on("data", (chunk) => chunks.push(chunk));
        stream.on("trailers", (t) => (headers = { ...headers, ...t }));
        stream.on("error", reject);
        stream.on("end", () => {
          const status = headers["grpc-status"];
          if (status !== undefined && status !== "0") {
            const why = decodeURIComponent(headers["grpc-message"] ?? "");
            reject(new Error(`ldk ${method}: grpc-status ${status} ${why}`));
            return;
          }
          const body = Buffer.concat(chunks);
          resolve(decode(body.subarray(5, 5 + body.readUInt32BE(1))));
        });
        stream.end(frame);
      });
    } finally {
      session.close();
    }
  }

  async nodeId() {
    return str(await this.#call("GetNodeInfo", Buffer.alloc(0)), 1);
  }

  /** Usable channels, with what each can send now. */
  async channels() {
    const answer = await this.#call("ListChannels", Buffer.alloc(0));
    return (answer[1] ?? []).map((raw) => {
      const c = decode(raw);
      return {
        channelId: str(c, 1),
        counterparty: str(c, 2),
        usable: num(c, 14) === 1n,
      };
    });
  }

  /** A fixed-amount BOLT11 invoice. */
  async invoice(amountSat, description = "swap-sdk live test") {
    const message = Buffer.concat([
      uint(1, BigInt(amountSat) * 1000n),
      bytes(2, bytes(1, description)),
      uint(3, 3600),
    ]);
    const answer = await this.#call("Bolt11Receive", message);
    return { invoice: str(answer, 1), paymentHash: str(answer, 2) };
  }

  /**
   * An invoice for a payment hash whose preimage this node does not know: an
   * HTLC for it is held until `fail` (or a claim) settles it.
   */
  async invoiceForHash(
    amountSat,
    paymentHashHex,
    description = "swap-sdk live test",
  ) {
    const message = Buffer.concat([
      uint(1, BigInt(amountSat) * 1000n),
      bytes(2, bytes(1, description)),
      uint(3, 3600),
      bytes(4, paymentHashHex),
    ]);
    return str(await this.#call("Bolt11ReceiveForHash", message), 1);
  }

  /** Reject the held HTLC for `paymentHashHex`, failing the sender's payment. */
  async fail(paymentHashHex) {
    await this.#call("Bolt11FailForHash", bytes(1, paymentHashHex));
  }

  /** Start paying `invoice`; resolves to the payment id at once. */
  async pay(invoice) {
    return str(await this.#call("Bolt11Send", bytes(1, invoice)), 1);
  }

  /** `pending`, `succeeded` or `failed`, and the preimage once known. */
  async payment(paymentId) {
    const answer = await this.#call("GetPaymentDetails", bytes(1, paymentId));
    const payment = decode(answer[1]?.[0] ?? Buffer.alloc(0));
    return { status: PAYMENT_STATUS[Number(num(payment, 5))] };
  }
}
