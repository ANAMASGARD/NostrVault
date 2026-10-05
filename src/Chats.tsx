import { conversationViews, type LogicalMessage } from "./chats";

export function Chats({ messages }: { messages: LogicalMessage[] }) {
  const views = conversationViews(messages);
  return (
    <section aria-labelledby="chats-title">
      <h2 id="chats-title">Conversations</h2>
      <p>
        Verified text comes from an authorized signer check of captured wraps.
        Pending and failed decryption stay separate. Deleted or expired bodies
        are hidden. This list does not send messages or load remote profiles.
      </p>
      {views.length === 0 ? (
        <p>No decrypted conversations in this vault yet.</p>
      ) : (
        <ul>
          {views.map((view) => (
            <li key={view.id}>
              <strong>{view.label}</strong>
              <ol>
                {view.messages.map((message) => (
                  <li key={message.rumorId}>
                    {message.visible
                      ? message.body
                      : message.state === "pending"
                        ? "Decryption pending"
                        : message.state === "failed"
                          ? "Decryption failed"
                          : "Hidden by deletion or expiration"}
                  </li>
                ))}
              </ol>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
