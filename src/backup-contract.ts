import { object } from "./vault-contract";
export type BackupAction =
  | { kind: "read"; offset: number }
  | { kind: "check_source" }
  | { kind: "capture" | "restore"; relay: string; approved: boolean };
export type BackupOutput = {
  requestId: string;
  binding: { vaultId: string; token: string; generation: number };
  account: string | null;
  source: string;
  capturedAt: number;
  count: number;
  suppressed: number;
  excluded: number;
  notes: { id: string; content: string; createdAt: number }[];
  outcome: string;
  restore: null | {
    destination: string;
    checkedAt: number;
    attempted: number;
    acknowledged: number;
    rejected: number;
    verified: number;
    verificationComplete: boolean;
  };
};
function text(v: unknown, max = 2048): string {
  if (typeof v !== "string" || v.length > max) throw new Error("malformed");
  return v;
}
function count(v: unknown): number {
  if (typeof v !== "number" || !Number.isSafeInteger(v) || v < 0)
    throw new Error("malformed");
  return v;
}
export function parseBackup(v: unknown): BackupOutput {
  if (
    !object(v) ||
    !object(v.binding) ||
    !Array.isArray(v.notes) ||
    v.notes.length > 16
  )
    throw new Error("malformed");
  let restore: BackupOutput["restore"] = null;
  if (v.restore !== null) {
    if (
      !object(v.restore) ||
      typeof v.restore.verificationComplete !== "boolean"
    )
      throw new Error("malformed");
    restore = {
      destination: text(v.restore.destination),
      checkedAt: count(v.restore.checkedAt),
      attempted: count(v.restore.attempted),
      acknowledged: count(v.restore.acknowledged),
      rejected: count(v.restore.rejected),
      verified: count(v.restore.verified),
      verificationComplete: v.restore.verificationComplete,
    };
  }
  return {
    requestId: text(v.requestId, 64),
    binding: {
      vaultId: text(v.binding.vaultId, 64),
      token: text(v.binding.token, 64),
      generation: count(v.binding.generation),
    },
    account: v.account === null ? null : text(v.account, 64),
    source: text(v.source),
    capturedAt: count(v.capturedAt),
    count: count(v.count),
    suppressed: count(v.suppressed),
    excluded: count(v.excluded),
    notes: v.notes.map((n: unknown) => {
      if (!object(n)) throw new Error("malformed");
      return {
        id: text(n.id, 64),
        content: text(n.content, 16384),
        createdAt: count(n.createdAt),
      };
    }),
    outcome: text(v.outcome, 64),
    restore,
  };
}
