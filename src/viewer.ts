import type { LogicalMessage } from "./chats";

export function visibleText(value: string): string {
  return value.replace(/<[^>]*>/g, "").slice(0, 4000);
}

export function searchMessages(
  messages: LogicalMessage[],
  query: string,
): LogicalMessage[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return messages.filter((message) => message.visible);
  return messages.filter(
    (message) => message.visible && message.body.toLowerCase().includes(needle),
  );
}
