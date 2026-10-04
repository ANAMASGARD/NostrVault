export const MAX_EVENT = 65_536;
export const MAX_PAYLOAD = 1_048_576;
export type FoundationControl = {
  version: 1;
  requestId: string;
  operation: "foundation_proof";
  eventLength: number;
  payloadLength: number;
};
export type FoundationRequest = FoundationControl & {
  phase: "prepare" | "reopen";
  store: string;
  event: Uint8Array;
  payload: Uint8Array;
};
export type FoundationResult = {
  eventId: string;
  payloadBytes: number;
  storageReopened: boolean;
  cryptoVerified: boolean;
};
export type FoundationResponse = {
  version: 1;
  requestId: string;
  result: FoundationResult | null;
  error: { code: string } | null;
};
export function isFoundationRequest(
  value: unknown,
): value is FoundationRequest {
  if (typeof value !== "object" || value === null) return false;
  return (
    "version" in value &&
    value.version === 1 &&
    "requestId" in value &&
    typeof value.requestId === "string" &&
    /^[a-zA-Z0-9_-]{1,64}$/.test(value.requestId) &&
    "operation" in value &&
    value.operation === "foundation_proof" &&
    "phase" in value &&
    (value.phase === "prepare" || value.phase === "reopen") &&
    "store" in value &&
    typeof value.store === "string" &&
    /^nostrvault-foundation-[a-zA-Z0-9-]{1,64}$/.test(value.store) &&
    "event" in value &&
    value.event instanceof Uint8Array &&
    value.event.byteLength <= MAX_EVENT &&
    "payload" in value &&
    value.payload instanceof Uint8Array &&
    value.payload.byteLength <= MAX_PAYLOAD &&
    "eventLength" in value &&
    value.eventLength === value.event.byteLength &&
    "payloadLength" in value &&
    value.payloadLength === value.payload.byteLength
  );
}
export class FoundationFailure extends Error {
  constructor(public readonly code: string) {
    super(`Foundation operation failed: ${code}`);
  }
}
export function parseFoundationResponse(
  value: unknown,
  requestId: string,
): FoundationResult {
  if (
    typeof value !== "object" ||
    value === null ||
    !("version" in value) ||
    value.version !== 1 ||
    !("requestId" in value) ||
    value.requestId !== requestId ||
    !("error" in value) ||
    !("result" in value)
  )
    throw new Error("Foundation response rejected");
  if (value.error !== null) {
    const failure = value.error;
    if (
      value.result === null &&
      typeof failure === "object" &&
      failure !== null &&
      "code" in failure &&
      typeof failure.code === "string" &&
      /^(malformed|unsupported|limit|invalid_id|invalid_signature|out_of_scope|authentication|crypto|crypto_work|storage|cancelled|randomness|busy|foundation_failed)$/.test(
        failure.code,
      )
    )
      throw new FoundationFailure(failure.code);
    throw new Error("Foundation response rejected");
  }
  const result = value.result;
  if (
    typeof result !== "object" ||
    result === null ||
    !("eventId" in result) ||
    typeof result.eventId !== "string" ||
    !/^[a-f0-9]{64}$/.test(result.eventId) ||
    !("payloadBytes" in result) ||
    typeof result.payloadBytes !== "number" ||
    !Number.isInteger(result.payloadBytes) ||
    result.payloadBytes < 0 ||
    result.payloadBytes > MAX_PAYLOAD ||
    !("storageReopened" in result) ||
    typeof result.storageReopened !== "boolean" ||
    !("cryptoVerified" in result) ||
    result.cryptoVerified !== true
  )
    throw new Error("Foundation response rejected");
  return {
    eventId: result.eventId,
    payloadBytes: result.payloadBytes,
    storageReopened: result.storageReopened,
    cryptoVerified: result.cryptoVerified,
  };
}
