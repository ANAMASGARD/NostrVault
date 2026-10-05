import { expect, it } from "vitest";
import { parseBackup } from "./backup-contract";
it("rejects malformed or unbounded public-note pages and verification claims", () => {
  const base = {
    requestId: "test",
    binding: { vaultId: "v", token: "t", generation: 1 },
    account: null,
    source: "",
    capturedAt: 0,
    count: 0,
    suppressed: 0,
    excluded: 0,
    notes: [],
    outcome: "offline_ready",
    restore: null,
  };
  expect(parseBackup(base).notes).toEqual([]);
  expect(() =>
    parseBackup({
      ...base,
      notes: Array.from({ length: 17 }, () => ({
        id: "x",
        content: "text",
        createdAt: 1,
      })),
    }),
  ).toThrow();
  expect(() => parseBackup({ ...base, count: NaN })).toThrow();
  expect(() => parseBackup({ ...base, restore: { verified: 1 } })).toThrow();
});
