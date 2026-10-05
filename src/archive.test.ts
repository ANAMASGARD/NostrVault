import { expect, it } from "vitest";
import { archiveFilename, parseArchiveManifest } from "./archive";

it("accepts a versioned manifest and rejects traversal names", () => {
  const account = "ab".repeat(32);
  expect(
    parseArchiveManifest({
      version: 1,
      snapshotId: "snap1",
      account,
      eventCount: 2,
      includeConversations: true,
    }).snapshotId,
  ).toBe("snap1");
  expect(archiveFilename("20261005", "snap1")).toBe(
    "nostrvault-20261005-snap1.nvarchive",
  );
  expect(() => archiveFilename("..", "snap1")).toThrow();
  expect(() =>
    parseArchiveManifest({
      version: 2,
      snapshotId: "snap1",
      account,
      eventCount: 1,
      includeConversations: false,
    }),
  ).toThrow();
});
