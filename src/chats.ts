export type DecryptState = "pending" | "failed" | "verified";

export type LogicalMessage = {
  rumorId: string;
  outerIds: string[];
  sender: string;
  createdAt: number;
  participants: string[];
  body: string;
  state: DecryptState;
  visible: boolean;
};

export type ConversationView = {
  id: string;
  label: string;
  messages: LogicalMessage[];
};

const STATES: DecryptState[] = ["pending", "failed", "verified"];

function text(value: unknown, max: number): string {
  if (typeof value !== "string" || value.length === 0 || value.length > max)
    throw new Error("malformed");
  return value;
}

export function projectMessages(input: unknown): LogicalMessage[] {
  if (!Array.isArray(input) || input.length > 256) throw new Error("malformed");
  const grouped = new Map<string, LogicalMessage>();
  for (const item of input) {
    if (!item || typeof item !== "object") throw new Error("malformed");
    const row = item as Record<string, unknown>;
    const rumorId = text(row.rumorId, 128);
    const outerId = text(row.outerId, 128);
    const sender = text(row.sender, 128);
    if (sender !== text(row.claimedSender, 128)) throw new Error("malformed");
    if (
      typeof row.createdAt !== "number" ||
      !Number.isSafeInteger(row.createdAt)
    )
      throw new Error("malformed");
    if (!Array.isArray(row.participants) || row.participants.length > 16)
      throw new Error("malformed");
    const participants = row.participants.map((part) => text(part, 128)).sort();
    const state = text(row.state, 16);
    if (!STATES.includes(state as DecryptState)) throw new Error("malformed");
    const decrypt = state as DecryptState;
    const deleted = row.deleted === true;
    const expired = row.expired === true;
    const visible = decrypt === "verified" && !deleted && !expired;
    const body = visible ? text(row.body, 16384) : "";
    const existing = grouped.get(rumorId);
    if (existing) {
      if (
        existing.sender !== sender ||
        existing.createdAt !== row.createdAt ||
        existing.participants.join(":") !== participants.join(":")
      )
        throw new Error("malformed");
      if (!existing.outerIds.includes(outerId)) {
        existing.outerIds.push(outerId);
        existing.outerIds.sort();
      }
      if (decrypt === "verified") {
        existing.state = decrypt;
        existing.body = body;
        existing.visible = visible;
      }
    } else {
      grouped.set(rumorId, {
        rumorId,
        outerIds: [outerId],
        sender,
        createdAt: row.createdAt,
        participants,
        body,
        state: decrypt,
        visible,
      });
    }
  }
  return [...grouped.values()];
}

export function conversationViews(
  messages: LogicalMessage[],
  aliases: Record<string, string> = {},
): ConversationView[] {
  const groups = new Map<string, ConversationView>();
  for (const message of messages) {
    const id = message.participants.join(":");
    const label = message.participants
      .map((key) => aliases[key] || `${key.slice(0, 8)}…`)
      .join(", ");
    const current = groups.get(id) ?? { id, label, messages: [] };
    current.messages.push(message);
    groups.set(id, current);
  }
  return [...groups.values()].map((view) => ({
    ...view,
    messages: [...view.messages].sort((a, b) => a.createdAt - b.createdAt),
  }));
}
