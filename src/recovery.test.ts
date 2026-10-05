import { expect, it } from "vitest";
import { classifyRestore, restoreSummary } from "./recovery";

it("keeps acknowledgement distinct from read-back and reports partial rejection", () => {
  expect(classifyRestore(true, false, false)).toBe("acknowledged");
  expect(classifyRestore(false, true, false)).toBe("verified");
  expect(classifyRestore(true, false, true)).toBe("rejected");
  expect(restoreSummary(2, 3)).toContain("2 of 3 eligible events");
});
