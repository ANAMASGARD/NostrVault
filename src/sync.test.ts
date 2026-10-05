import { expect, it } from "vitest";
import { parseSync } from "./sync";

it("accepts a checkpoint view and rejects unknown states", () => {
  expect(parseSync(null)).toBeNull();
  expect(
    parseSync({ id: "a|ws://r|k", state: "needs_auth", until: 10 })?.state,
  ).toBe("needs_auth");
  expect(() => parseSync({ id: "a", state: "done", until: 1 })).toThrow();
});
