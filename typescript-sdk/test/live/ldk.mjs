// A few ldk-server gRPC calls, enough to mint and pay BOLT11 invoices from
// the live suite: HTTP/2 from node:http2, the protobuf messages by hand, and
// ldk-server's `x-auth: HMAC <ts>:<hmac(api key, ts || body)>` header.
import { createHmac } from "node:crypto";
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

const str = (fields, n) => fields[n]?.[0]?.toString("utf8") ?? "";
const num = (fields, n) => fields[n]?.[0] ?? 0n;

export const PAYMENT_STATUS = ["pending", "succeeded", "failed"];

export class LdkServer {
  /**
   * @param {string} address `host:port` of the gRPC listener.
   * @param {string} apiKeyHex the node's `api_key` file, hex-encoded.
   * @param {string | Buffer} certPem the node's TLS certificate.
   */
  constructor(address, apiKeyHex, certPem) {
    this.address = address;
    this.apiKey = apiKeyHex;
    this.certPem = certPem;
  }

  async #call(method, message) {
    const frame = Buffer.alloc(5 + message.length);
    frame.writeUInt32BE(message.length, 1);
    message.copy(frame, 5);
    const timestamp = BigInt(Math.floor(Date.now() / 1000));
    const ts = Buffer.alloc(8);
    ts.writeBigUInt64BE(timestamp);
    const mac = createHmac("sha256", this.apiKey)
      .update(ts)
      .update(frame)
      .digest("hex");

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
          "x-auth": `HMAC ${timestamp}:${mac}`,
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
