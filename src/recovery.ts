export type RestoreOutcome =
  "acknowledged" | "verified" | "rejected" | "unknown";

export function classifyRestore(
  acknowledged: boolean,
  readBack: boolean,
  rejected: boolean,
): RestoreOutcome {
  if (rejected) return "rejected";
  if (readBack) return "verified";
  if (acknowledged) return "acknowledged";
  return "unknown";
}

export function restoreSummary(verified: number, eligible: number): string {
  return `${verified} of ${eligible} eligible events were returned and validated from the selected relay at the recorded check time.`;
}
