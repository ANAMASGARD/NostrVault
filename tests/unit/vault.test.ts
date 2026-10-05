import { describe, it, expect } from "vitest";
import {
  parseStatus,
  validUnicode,
  validatePasswords,
} from "../../src/vault-contract";
describe("vault boundaries", () => {
  it("preserves exact Unicode and rejects unmatched surrogates before transport", () => {
    for (const value of [" spaces stay ", "🔑 twelve keys 🔑", "é", "e\u0301"])
      expect(validUnicode(value)).toBe(true);
    for (const value of ["\ud800", "\udc00", "a\ud800b"])
      expect(() =>
        validatePasswords({ kind: "unlock", password: value }),
      ).toThrow();
    expect(() =>
      validatePasswords({ kind: "unlock", password: "x".repeat(1025) }),
    ).toThrow();
  });
  it("refuses protected data attached to locked or malformed status", () => {
    const status = {
      state: "locked",
      vaultId: "a".repeat(32),
      token: "b".repeat(32),
      generation: 0,
      revision: 1,
      setup: null,
    };
    expect(parseStatus(status).state).toBe("locked");
    expect(() =>
      parseStatus({
        ...status,
        setup: { version: 1, protected: true, lossAcknowledged: true },
      }),
    ).toThrow();
    for (const invalid of [
      { ...status, generation: -1 },
      { ...status, token: "" },
      { ...status, revision: Infinity },
    ])
      expect(() => parseStatus(invalid)).toThrow();
  });
});
