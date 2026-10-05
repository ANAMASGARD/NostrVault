export type ArchiveManifest = {
  version: number;
  snapshotId: string;
  account: string;
  eventCount: number;
  includeConversations: boolean;
};

function name(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 128 &&
    /^[A-Za-z0-9_-]+$/.test(value) &&
    !value.includes("..")
  );
}

export function parseArchiveManifest(value: unknown): ArchiveManifest {
  if (!value || typeof value !== "object") throw new Error("malformed");
  const row = value as Record<string, unknown>;
  if (row.version !== 1) throw new Error("unsupported");
  if (typeof row.snapshotId !== "string" || !name(row.snapshotId))
    throw new Error("malformed");
  if (typeof row.account !== "string" || !/^[0-9a-fA-F]{64}$/.test(row.account))
    throw new Error("malformed");
  if (
    typeof row.eventCount !== "number" ||
    !Number.isSafeInteger(row.eventCount) ||
    row.eventCount < 0 ||
    row.eventCount > 256
  )
    throw new Error("malformed");
  if (typeof row.includeConversations !== "boolean")
    throw new Error("malformed");
  return {
    version: 1,
    snapshotId: row.snapshotId,
    account: row.account,
    eventCount: row.eventCount,
    includeConversations: row.includeConversations,
  };
}

export function archiveFilename(date: string, snapshotId: string): string {
  if (!name(date) || !name(snapshotId)) throw new Error("malformed");
  return `nostrvault-${date}-${snapshotId}.nvarchive`;
}
