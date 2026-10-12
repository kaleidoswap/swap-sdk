import type { BtcLikeTransaction, SwapScript, TxParams } from "./index.js";

export type SpendAction =
  "refundSubmarine" | "refundChain" | "claimReverse" | "claimChain";
export type SpendPath = "cooperative" | "script";
export interface SwapTimeout {
  timeoutBlockHeight: number | bigint;
}
export interface ChainSwapTiming {
  lockupDetails: SwapTimeout;
  claimDetails: SwapTimeout;
}
export type SwapTiming = SwapTimeout | ChainSwapTiming;
export type SpendWaitReason =
  | "unknown-status"
  | "already-spending"
  | "completed"
  | "waiting-for-lockup"
  | "waiting-for-confirmation"
  | "waiting-for-timeout"
  | "claim-timeout"
  | "invoice-in-flight"
  | "missing-swap-auth"
  | "missing-cooperative-context";
export type SpendEligibility =
  | { eligible: true; path: SpendPath }
  | { eligible: false; reason: SpendWaitReason };

function invalid(message: string): never {
  throw Object.assign(new Error(message), { code: "InvalidArgument" });
}

function mode(value: string | undefined): void {
  if (value !== undefined && !["auto", "cooperative", "script"].includes(value))
    invalid("mode must be auto, cooperative or script");
}

function height(value: number | bigint, name: string): number {
  if (typeof value === "bigint") {
    if (value < 0n || value >= 500_000_000n)
      invalid(`${name} must be a block height`);
    return Number(value);
  }
  if (!Number.isSafeInteger(value) || value < 0 || value >= 500_000_000)
    invalid(`${name} must be a block height`);
  return value;
}

function timeout(response: SwapTiming, side: "lockup" | "claim"): number {
  const details =
    "timeoutBlockHeight" in response
      ? response
      : side === "lockup"
        ? response.lockupDetails
        : response.claimDetails;
  const result = height(details.timeoutBlockHeight, "timeoutBlockHeight");
  if (result === 0) invalid("timeoutBlockHeight must be positive");
  return result;
}

const wait = (reason: SpendWaitReason): SpendEligibility => ({
  eligible: false,
  reason,
});

/** Advisory snapshot check. tipHeight is the taker's lockup chain tip. */
export function canRefund(
  swapStatus: string,
  tipHeight: number | bigint,
  createResponse: SwapTiming,
  options: {
    kind?: "submarine" | "chain";
    mode?: "auto" | "cooperative" | "script";
  } = {},
): SpendEligibility {
  mode(options.mode);
  if (
    options.kind !== undefined &&
    !["submarine", "chain"].includes(options.kind)
  )
    invalid("refund kind must be submarine or chain");
  const tip = height(tipHeight, "tipHeight");
  const mature = tip >= timeout(createResponse, "lockup");
  if (
    ["transaction.claimed", "invoice.paid", "invoice.settled"].includes(
      swapStatus,
    )
  )
    return wait("completed");
  if (swapStatus === "transaction.claim.pending")
    return wait("already-spending");
  if (swapStatus === "invoice.pending") return wait("invoice-in-flight");
  const failed = [
    "invoice.failedToPay",
    "transaction.lockupFailed",
    "swap.expired",
  ];
  const funded = [
    "transaction.mempool",
    "transaction.confirmed",
    "invoice.set",
  ];
  if (options.kind === "chain") {
    failed.push("transaction.failed", "transaction.refunded");
    funded.push(
      "transaction.zeroconf.rejected",
      "transaction.server.mempool",
      "transaction.server.confirmed",
    );
  }
  if (swapStatus === "swap.created") return wait("waiting-for-lockup");
  if (!failed.includes(swapStatus) && !funded.includes(swapStatus))
    return wait("unknown-status");
  if (options.mode === "script")
    return mature
      ? { eligible: true, path: "script" }
      : wait("waiting-for-timeout");
  // At maturity the next block can include nLockTime=timeout. No signature
  // refusal is caught or reinterpreted as authorization for another path.
  if (mature && options.mode !== "cooperative")
    return { eligible: true, path: "script" };
  return failed.includes(swapStatus)
    ? { eligible: true, path: "cooperative" }
    : wait("waiting-for-timeout");
}

/** Advisory script-path check. tipHeight is the claim chain tip; 0-conf is opt-in. */
export function canClaim(
  swapStatus: string,
  tipHeight: number | bigint,
  createResponse: SwapTiming,
  options: { kind?: "reverse" | "chain"; allowZeroConf?: boolean } = {},
): SpendEligibility {
  if (
    options.kind !== undefined &&
    !["reverse", "chain"].includes(options.kind)
  )
    invalid("claim kind must be reverse or chain");
  if (
    options.allowZeroConf !== undefined &&
    typeof options.allowZeroConf !== "boolean"
  )
    invalid("allowZeroConf must be a boolean");
  const tip = height(tipHeight, "tipHeight");
  const deadline = timeout(createResponse, "claim");
  if (
    [
      "invoice.settled",
      "transaction.claimed",
      "transaction.refunded",
      "invoice.expired",
      "swap.expired",
      "transaction.failed",
    ].includes(swapStatus)
  )
    return wait("completed");
  if (swapStatus === "transaction.claim.pending")
    return wait("already-spending");
  const confirmed =
    options.kind === "chain"
      ? "transaction.server.confirmed"
      : "transaction.confirmed";
  const mempool =
    options.kind === "chain"
      ? "transaction.server.mempool"
      : "transaction.mempool";
  if (swapStatus === confirmed || swapStatus === mempool) {
    if (tip >= deadline) return wait("claim-timeout");
    if (swapStatus === mempool && !options.allowZeroConf)
      return wait("waiting-for-confirmation");
    return { eligible: true, path: "script" };
  }
  const pending = [
    "swap.created",
    "minerfee.paid",
    "transaction.mempool",
    "transaction.confirmed",
    "transaction.zeroconf.rejected",
  ];
  return wait(
    pending.includes(swapStatus) ? "waiting-for-lockup" : "unknown-status",
  );
}

/** Persist the transaction's hex/txid before broadcast; restore it for retries. */
export interface SignedSwapSpend {
  kind: "signed";
  swapId: string;
  action: SpendAction;
  path: SpendPath;
  transaction: BtcLikeTransaction;
}
export type SwapSpendResult =
  SignedSwapSpend | { kind: "not-yet"; reason: SpendWaitReason };
export interface SwapSpendOptions {
  status: string;
  /** Refund: lockup chain. Claim: receiving chain. Fetch a fresh trusted tip. */
  tipHeight: number | bigint;
  createResponse: SwapTiming;
  /** Already validated/reconstructed script for the side being spent. */
  script: Pick<
    SwapScript,
    "constructClaim" | "constructRefund" | "constructCooperativeClaim"
  >;
  params: Omit<TxParams, "cooperative">;
  /** A recovered previous result wins over current status/fees; never re-sign. */
  previous?: SignedSwapSpend;
}
export interface ClaimSwapOptions extends SwapSpendOptions {
  preimageHex: string;
  allowZeroConf?: boolean;
  /** Defaults to cooperative for reverse; chain needs the extra context below. */
  mode?: "auto" | "cooperative" | "script";
}
export interface ClaimChainOptions extends ClaimSwapOptions {
  lockupScript?: SwapScript;
  refundKeysSecretHex?: string;
}
export interface RefundSwapOptions extends SwapSpendOptions {
  /** Auto uses cooperative before timeout, script once the CLTV is mature. */
  mode?: "auto" | "cooperative" | "script";
}

function previous(
  options: SwapSpendOptions,
  action: SpendAction,
): SignedSwapSpend | undefined {
  if (options.previous) {
    if (
      options.previous.kind !== "signed" ||
      options.previous.swapId !== options.params.swapId ||
      options.previous.action !== action
    )
      invalid("previous spend must match swapId and action");
    return options.previous;
  }
}

function fee(params: SwapSpendOptions["params"]): void {
  const { feeSatPerVb: rate, feeAbsoluteSat: absolute } = params;
  if ((rate === undefined) === (absolute === undefined))
    invalid("provide exactly one feeSatPerVb or feeAbsoluteSat");
  if (rate !== undefined && (!Number.isFinite(rate) || rate <= 0))
    invalid("feeSatPerVb must be positive and finite");
  if (
    absolute !== undefined &&
    (!Number.isSafeInteger(absolute) || absolute <= 0)
  )
    invalid("feeAbsoluteSat must be a positive safe integer");
}

async function refund(
  options: RefundSwapOptions,
  action: "refundSubmarine" | "refundChain",
): Promise<SwapSpendResult> {
  const saved = previous(options, action);
  if (saved) return saved;
  const eligibility = canRefund(
    options.status,
    options.tipHeight,
    options.createResponse,
    {
      kind: action === "refundChain" ? "chain" : "submarine",
      mode: options.mode,
    },
  );
  if (!eligibility.eligible)
    return { kind: "not-yet", reason: eligibility.reason };
  if (eligibility.path === "cooperative" && !options.params.swapAuth)
    return { kind: "not-yet", reason: "missing-swap-auth" };
  fee(options.params);
  const transaction = await options.script.constructRefund({
    ...options.params,
    cooperative: eligibility.path === "cooperative",
  });
  return {
    kind: "signed",
    swapId: options.params.swapId,
    action,
    path: eligibility.path,
    transaction,
  };
}

export const refundSubmarineSwap = (
  options: RefundSwapOptions,
): Promise<SwapSpendResult> => refund(options, "refundSubmarine");
export const refundChainSwap = (
  options: RefundSwapOptions,
): Promise<SwapSpendResult> => refund(options, "refundChain");

async function claim(
  options: ClaimChainOptions,
  action: "claimReverse" | "claimChain",
): Promise<SwapSpendResult> {
  const saved = previous(options, action);
  if (saved) return saved;
  mode(options.mode);
  const eligibility = canClaim(
    options.status,
    options.tipHeight,
    options.createResponse,
    {
      kind: action === "claimChain" ? "chain" : "reverse",
      allowZeroConf: options.allowZeroConf,
    },
  );
  if (!eligibility.eligible)
    return { kind: "not-yet", reason: eligibility.reason };
  const chain = action === "claimChain";
  const context =
    options.lockupScript !== undefined &&
    options.refundKeysSecretHex !== undefined;
  const partialContext =
    options.lockupScript !== undefined ||
    options.refundKeysSecretHex !== undefined;
  if (
    chain &&
    options.mode !== "script" &&
    !context &&
    (options.mode === "cooperative" || partialContext)
  )
    return { kind: "not-yet", reason: "missing-cooperative-context" };
  const cooperative = options.mode !== "script" && (!chain || context);
  fee(options.params);
  const params = { ...options.params, cooperative };
  const transaction =
    chain && cooperative
      ? await options.script.constructCooperativeClaim(
          options.preimageHex,
          params,
          options.lockupScript!,
          options.refundKeysSecretHex!,
        )
      : await options.script.constructClaim(options.preimageHex, params);
  return {
    kind: "signed",
    swapId: params.swapId,
    action,
    path: cooperative ? "cooperative" : "script",
    transaction,
  };
}

export const claimReverseSwap = (
  options: ClaimSwapOptions,
): Promise<SwapSpendResult> => claim(options, "claimReverse");
export const claimChainSwap = (
  options: ClaimChainOptions,
): Promise<SwapSpendResult> => claim(options, "claimChain");
