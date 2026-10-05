import { useState } from "react";
import type { LogicalMessage } from "./chats";
import { searchMessages, visibleText } from "./viewer";

export function Viewer({ messages }: { messages: LogicalMessage[] }) {
  const [query, setQuery] = useState("");
  const hits = searchMessages(messages, query);
  return (
    <section aria-labelledby="viewer-title">
      <h2 id="viewer-title">Offline reading</h2>
      <p>
        Search stays in memory and clears when the vault locks. Opening a
        conversation does not contact a relay or load remote images.
      </p>
      <label>
        Search saved messages
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          autoComplete="off"
        />
      </label>
      <ul>
        {hits.map((message) => (
          <li key={message.rumorId}>{visibleText(message.body)}</li>
        ))}
      </ul>
    </section>
  );
}
