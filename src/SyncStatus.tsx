import type { SyncView } from "./sync";

const COPY: Record<SyncView["state"], string> = {
  running: "Catch-up is running. Events are saved before the checkpoint moves.",
  paused: "Catch-up is paused after repeated failures.",
  needs_auth:
    "Catch-up is waiting for relay authentication on the same connection.",
  cancelled: "Catch-up was cancelled. Saved events stay in the vault.",
};

export function SyncStatus({ job }: { job: SyncView | null }) {
  return (
    <section aria-labelledby="sync-title">
      <h2 id="sync-title">Live capture</h2>
      {job ? (
        <p role="status">{COPY[job.state]}</p>
      ) : (
        <p>
          No live catch-up job is stored yet. Initial collection remains
          one-shot. Closing the window stops the engine unless keep-running is
          enabled. Readable history stays paused while locked. Encrypted capture
          while locked is opt-in and does not update the readable list.
          Autostart is not enabled. Android catch-up, when scheduled, is
          approximate and does not decrypt in the background.
        </p>
      )}
    </section>
  );
}
