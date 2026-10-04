import { describe, expect, it } from "vitest";
import { parseNativeRuntime } from "./runtime";

describe("runtime contract", () => {
  it.each(["linux", "android"])(
    "accepts a supported native host: %s",
    (platform) => {
      expect(parseNativeRuntime({ platform, version: "0.1.0" })).toEqual({
        platform,
        version: "0.1.0",
      });
    },
  );
  it.each([
    null,
    undefined,
    "linux",
    {},
    { platform: "linux" },
    { platform: "browser", version: "0.1.0" },
    { platform: "android", version: 1 },
    { platform: "linux", version: "<script>" },
  ])("rejects malformed native data: %j", (input) => {
    expect(() => parseNativeRuntime(input)).toThrow(
      "Invalid native runtime response",
    );
  });
});
