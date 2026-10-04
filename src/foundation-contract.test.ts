import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import metadata from "./foundation-fixture.json";
import {
  FoundationFailure,
  isFoundationRequest,
  parseFoundationResponse,
} from "./foundation-contract";

const request = {
  version: 1,
  requestId: "proof",
  operation: "foundation_proof",
  eventLength: 1,
  payloadLength: 1,
  event: new Uint8Array([1]),
  payload: new Uint8Array([2]),
  phase: "prepare",
  store: "nostrvault-foundation-proof",
};
const response = {
  version: 1,
  requestId: "proof",
  error: null,
  result: {
    eventId: "a".repeat(64),
    payloadBytes: 1,
    storageReopened: true,
    cryptoVerified: true,
  },
};
describe("foundation trust boundary", () => {
  it("accepts bounded binary requests and matching complete engine replies", () => {
    expect(isFoundationRequest(request)).toBe(true);
    expect(parseFoundationResponse(response, "proof")).toEqual(response.result);
    expect(metadata.payloadBytes).toBe(
      readFileSync("crates/vault-core/fixtures/payload.zip").length,
    );
  });
  it("preserves only known structured failure codes", () => {
    expect(() =>
      parseFoundationResponse(
        { ...response, result: null, error: { code: "crypto_work" } },
        "proof",
      ),
    ).toThrow(FoundationFailure);
    expect(() =>
      parseFoundationResponse(
        {
          ...response,
          result: null,
          error: { code: "private untrusted data" },
        },
        "proof",
      ),
    ).toThrow("response rejected");
  });
  it.each([
    null,
    {},
    { ...request, version: 2 },
    { ...request, operation: "other" },
    { ...request, payload: [2] },
    { ...request, eventLength: 2 },
    { ...request, payload: new Uint8Array(1048577), payloadLength: 1048577 },
    { ...request, requestId: "" },
  ])("rejects malformed or oversized control/binary data", (value) => {
    expect(isFoundationRequest(value)).toBe(false);
  });
  it.each([
    null,
    { ...response, version: 2 },
    { ...response, requestId: "stale" },
    { ...response, error: { code: "storage" } },
    { ...response, result: { ...response.result, payloadBytes: -1 } },
    { ...response, result: { ...response.result, cryptoVerified: false } },
  ])("rejects stale and malformed replies", (value) => {
    expect(() => parseFoundationResponse(value, "proof")).toThrow();
  });
});
