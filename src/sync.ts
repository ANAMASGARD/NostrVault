export type SyncState = "running" | "paused" | "needs_auth" | "cancelled";

export type SyncView = {
  id: string;
  state: SyncState;
  until: number;
};

const STATES: SyncState[] = ["running", "paused", "needs_auth", "cancelled"];

export function parseSync(value: unknown): SyncView | null {
  if (value == null) return null;
  if (!value || typeof value !== "object") throw new Error("malformed");
  const row = value as Record<string, unknown>;
  if (typeof row.id !== "string" || row.id.length === 0 || row.id.length > 768)
    throw new Error("malformed");
  if (typeof row.state !== "string" || !STATES.includes(row.state as SyncState))
    throw new Error("malformed");
  if (
    typeof row.until !== "number" ||
    !Number.isSafeInteger(row.until) ||
    row.until < 0
  )
    throw new Error("malformed");
  return { id: row.id, state: row.state as SyncState, until: row.until };
}
