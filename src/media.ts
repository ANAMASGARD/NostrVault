export type MediaState = "missing" | "stored" | "rejected";

export function mediaLabel(state: MediaState): string {
  switch (state) {
    case "missing":
      return "File URL only. Bytes are not in this vault.";
    case "stored":
      return "Encrypted file bytes are stored for offline preview.";
    case "rejected":
      return "File was rejected because the hash or size did not match.";
    default: {
      const never: never = state;
      return never;
    }
  }
}
