import { mediaLabel, type MediaState } from "./media";

export function MediaNote({ state }: { state: MediaState | null }) {
  return (
    <section aria-labelledby="media-title">
      <h2 id="media-title">Attachments</h2>
      <p>
        Downloads stay off until you choose a host. Previews use bytes already
        stored in the vault. Scripts in imported files are not run.
      </p>
      {state ? <p role="status">{mediaLabel(state)}</p> : null}
    </section>
  );
}
