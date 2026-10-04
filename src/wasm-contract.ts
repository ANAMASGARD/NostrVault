export type ProbeRequest = { requestId: string; input: number };
export type ProbeResponse =
  | { requestId: string; type: "result"; result: number }
  | { requestId: string; type: "error" };

export function isProbeRequest(value: unknown): value is ProbeRequest {
  return (
    typeof value === "object" &&
    value !== null &&
    "requestId" in value &&
    typeof value.requestId === "string" &&
    value.requestId.length > 0 &&
    value.requestId.length <= 64 &&
    "input" in value &&
    typeof value.input === "number" &&
    Number.isInteger(value.input) &&
    value.input >= 0 &&
    value.input <= 0xffffffff
  );
}

export function parseProbeResponse(value: unknown, requestId: string): number {
  if (
    typeof value !== "object" ||
    value === null ||
    !("requestId" in value) ||
    value.requestId !== requestId ||
    !("type" in value) ||
    value.type !== "result" ||
    !("result" in value) ||
    typeof value.result !== "number" ||
    !Number.isInteger(value.result) ||
    value.result < 0 ||
    value.result > 0xffffffff
  ) {
    throw new Error("WASM runtime failed or returned an invalid response");
  }
  return value.result;
}
