import { describe, expect, it } from "vitest";
import { isProbeRequest, parseProbeResponse } from "./wasm-contract";

describe("Worker probe boundary", () => {
  it("accepts a bounded u32 request and a matching typed result", () => {
    expect(isProbeRequest({ requestId: "probe", input: 42 })).toBe(true);
    expect(
      parseProbeResponse(
        { requestId: "probe", type: "result", result: 85 },
        "probe",
      ),
    ).toBe(85);
  });
  it.each([
    null,
    {},
    { requestId: "probe", input: -1 },
    { requestId: "probe", input: 2 ** 32 },
    { requestId: "probe", input: NaN },
  ])("rejects malformed requests: %j", (value) => {
    expect(isProbeRequest(value)).toBe(false);
  });
  it.each([
    null,
    { requestId: "stale", type: "result", result: 85 },
    { requestId: "probe", type: "error" },
    { requestId: "probe", type: "result", result: "85" },
  ])("rejects failed, stale or malformed replies: %j", (value) => {
    expect(() => parseProbeResponse(value, "probe")).toThrow();
  });
});
