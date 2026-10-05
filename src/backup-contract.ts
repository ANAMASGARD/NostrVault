import { object } from "./vault-contract";
export type CollectionProfile =
  "public_history" | "legacy_direct_messages" | "gift_wraps";

export type DiscoverySuggestions = {
  history: string[];
  inbox: string[];
};

export type InitialJob = {
  version: number;
  profile: CollectionProfile;
  relay: string;
  consentRevision: number;
  state: string;
  lastAttempt: number;
};

export type BackupAction =
  | { kind: "read"; offset: number }
  | { kind: "check_source" }
  | { kind: "discover"; lookupRelays: string[] }
  | {
      kind: "capture";
      relay: string;
      profile?: CollectionProfile;
    }
  | { kind: "resume_collection" }
  | { kind: "cancel_collection" }
  | {
      kind: "start_initial_job";
      relay: string;
      profile?: CollectionProfile;
    }
  | { kind: "restore"; relay: string; approved: boolean };
export type BackupOutput = {
  requestId: string;
  binding: { vaultId: string; token: string; generation: number };
  account: string | null;
  source: string;
  capturedAt: number;
  count: number;
  suppressed: number;
  excluded: number;
  rejected: number;
  notes: { id: string; content: string; createdAt: number; kind: number }[];
  outcome: string;
  suggestions: DiscoverySuggestions;
  job: InitialJob | null;
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
function relayList(v: unknown): string[] {
  if (!Array.isArray(v) || v.length > 16) throw new Error("malformed");
  return v.map((s) => text(s));
}
function parseSuggestions(v: unknown): DiscoverySuggestions {
  if (!object(v)) return { history: [], inbox: [] };
  return {
    history: relayList(v.history),
    inbox: relayList(v.inbox),
  };
}
function parseJob(v: unknown): InitialJob | null {
  if (v === null || v === undefined) return null;
  if (!object(v)) throw new Error("malformed");
  return {
    version: count(v.version),
    profile: text(v.profile, 64) as CollectionProfile,
    relay: text(v.relay),
    consentRevision: count(v.consentRevision),
    state: text(v.state, 64),
    lastAttempt: count(v.lastAttempt),
  };
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
    rejected: count(v.rejected ?? 0),
    notes: v.notes.map((n: unknown) => {
      if (!object(n)) throw new Error("malformed");
      return {
        id: text(n.id, 64),
        content: text(n.content, 16384),
        createdAt: count(n.createdAt),
        kind: count(n.kind ?? 1),
      };
    }),
    outcome: text(v.outcome, 64),
    suggestions: parseSuggestions(v.suggestions),
    job: parseJob(v.job),
    restore,
  };
}
