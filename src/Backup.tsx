import { useEffect, useRef, useState } from "react";
import type { BackupAction, BackupOutput } from "./backup-contract";
import type { VaultRuntime } from "./vault-runtime";

export function Backup({ host }: { host: VaultRuntime }) {
  const [linux, setLinux] = useState(false);
  const [view, setView] = useState<BackupOutput | null>(null);
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [approved, setApproved] = useState(false);
  const [lookupDamus, setLookupDamus] = useState(true);
  const [lookupNos, setLookupNos] = useState(true);
  const [profile, setProfile] = useState<
    "public_history" | "legacy_direct_messages" | "gift_wraps"
  >("public_history");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [offset, setOffset] = useState(0);
  const live = useRef(false);
  const running = useRef(false);

  useEffect(() => {
    let active = true;
    live.current = true;
    if (__NATIVE_BUILD__)
      void import("@tauri-apps/api/core")
        .then(async ({ invoke }) => {
          const info = await invoke<{ platform: string }>("runtime_info");
          if (active && info.platform === "linux") {
            setLinux(true);
            const next = await host.backup({ kind: "read", offset: 0 });
            if (active) {
              setView(next);
              setSource(next.source);
            }
          }
        })
        .catch(() => {
          if (active)
            setError(
              "Saved events could not be opened. Lock and unlock to retry.",
            );
        });
    return () => {
      active = false;
      live.current = false;
    };
  }, [host]);

  async function run(action: BackupAction) {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError("");
    try {
      const result = await host.backup(action);
      if (live.current) {
        setView(result);
        setOffset(action.kind === "read" ? action.offset : 0);
      }
    } catch {
      if (live.current)
        setError(
          "Operation did not complete. Saved data remains local. Save capture grants in Identity, confirm the relay URL, or retry after unlock.",
        );
    } finally {
      running.current = false;
      if (live.current) setBusy(false);
    }
  }

  if (!linux) return null;

  const historyHints = view?.suggestions.history ?? [];
  const inboxHints = view?.suggestions.inbox ?? [];

  return (
    <section aria-labelledby="backup-title">
      <h3 id="backup-title">Initial collection (one-shot)</h3>
      <p>
        Milestone 05 · Linux only. Continuous backup is not enabled yet. Save
        lookup and capture relay grants in Identity before collecting. Public
        history, legacy DMs, and gift wraps are stored as raw events; only kind
        1 notes can be restored in this preview.
      </p>
      {view?.job && (
        <p role="status">
          Initial job: {view.job.state} · {view.job.relay} · profile{" "}
          {view.job.profile}
        </p>
      )}
      <fieldset>
        <legend>Lookup relays (max 3, metadata only)</legend>
        <p>Add the same URLs under Identity → Advanced before discovering.</p>
        <label>
          <input
            type="checkbox"
            checked={lookupDamus}
            onChange={(e) => setLookupDamus(e.target.checked)}
            disabled={busy}
          />
          wss://relay.damus.io
        </label>
        <label>
          <input
            type="checkbox"
            checked={lookupNos}
            onChange={(e) => setLookupNos(e.target.checked)}
            disabled={busy}
          />
          wss://nos.lol
        </label>
        <button
          type="button"
          disabled={busy || (!lookupDamus && !lookupNos)}
          onClick={() => {
            const lookupRelays: string[] = [];
            if (lookupDamus) lookupRelays.push("wss://relay.damus.io");
            if (lookupNos) lookupRelays.push("wss://nos.lol");
            void run({ kind: "discover", lookupRelays });
          }}
        >
          Discover relay hints
        </button>
        {historyHints.length > 0 && (
          <>
            <h4>Public-history relays</h4>
            <ul>
              {historyHints.map((relay) => (
                <li key={relay}>
                  {relay}{" "}
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => setSource(relay)}
                  >
                    Use as capture source
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
        {inboxHints.length > 0 && (
          <>
            <h4>Private-message inboxes</h4>
            <ul>
              {inboxHints.map((relay) => (
                <li key={relay}>{relay}</li>
              ))}
            </ul>
          </>
        )}
      </fieldset>
      <label>
        Collection profile
        <select
          value={profile}
          onChange={(e) =>
            setProfile(
              e.target.value as
                "public_history" | "legacy_direct_messages" | "gift_wraps",
            )
          }
          disabled={busy}
        >
          <option value="public_history">Public history</option>
          <option value="legacy_direct_messages">Legacy DMs (NIP-04)</option>
          <option value="gift_wraps">Gift wraps (NIP-17)</option>
        </select>
      </label>
      {error && <p role="alert">{error}</p>}
      {view?.outcome === "waiting_for_approval" && (
        <p role="status">
          Relay authentication needs signer approval on this connection. Finish
          approval in Identity, then resume.
        </p>
      )}
      {view?.outcome === "waiting_for_approval" && (
        <button
          type="button"
          disabled={busy}
          onClick={() => void run({ kind: "resume_collection" })}
        >
          Resume after signer approval
        </button>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run({
            kind: "start_initial_job",
            relay: source,
            profile,
          });
        }}
      >
        <label>
          Capture source relay
          <input
            type="url"
            required
            value={source}
            onChange={(e) => setSource(e.target.value)}
            placeholder="wss://relay.example"
            disabled={busy}
          />
        </label>
        <p>
          The URL must appear in Identity capture grants. A confirmed account
          can collect public history while the signer is disconnected; NIP-42
          still needs the signer when the relay requires it.
        </p>
        <button disabled={busy || !source}>Run initial collection</button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void run({ kind: "capture", relay: source, profile })}
        >
          Retry capture
        </button>
      </form>
      <p role="status">
        Outcome: {view?.outcome ?? "unknown"}. Rejected events:{" "}
        {view?.rejected ?? 0}.
      </p>
      <p>
        Stored events: <strong>{view?.count ?? 0}</strong> (kind 1 previews
        below).
      </p>
      {view?.capturedAt ? (
        <p>
          Last capture: {new Date(view.capturedAt * 1000).toLocaleString()}.
          Incomplete or saturated responses are limitations, not proof of full
          history.
        </p>
      ) : null}
      <p>
        Deletion targets: {view?.suppressed ?? 0}. Excluded:{" "}
        {view?.excluded ?? 0}.
      </p>
      <button
        type="button"
        disabled={busy || !view?.source}
        onClick={() => void run({ kind: "check_source" })}
      >
        Check source connection
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={() => void run({ kind: "read", offset: 0 })}
      >
        Read offline
      </button>
      <ol aria-label="Saved kind-1 previews">
        {view?.notes
          .filter((n) => n.kind === 1)
          .map((note) => (
            <li key={note.id}>
              <p className="backup-note">{note.content}</p>
              <small>
                {new Date(note.createdAt * 1000).toLocaleString()} · {note.id}
              </small>
            </li>
          ))}
      </ol>
      <button
        disabled={busy || offset === 0}
        onClick={() =>
          void run({ kind: "read", offset: Math.max(0, offset - 16) })
        }
      >
        Previous page
      </button>
      <button
        disabled={busy || offset + 16 >= (view?.count ?? 0)}
        onClick={() => void run({ kind: "read", offset: offset + 16 })}
      >
        Next page
      </button>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (approved)
            void run({ kind: "restore", relay: destination, approved });
        }}
      >
        <label>
          Recovery destination relay
          <input
            type="url"
            required
            value={destination}
            onChange={(e) => {
              setDestination(e.target.value);
              setApproved(false);
            }}
            placeholder="wss://recovery.example"
            disabled={busy}
          />
        </label>
        <label>
          <input
            type="checkbox"
            checked={approved}
            onChange={(e) => setApproved(e.target.checked)}
            disabled={busy}
          />
          I approve publishing eligible kind-1 notes to this destination for
          this recovery preview only.
        </label>
        <button disabled={busy || !approved || !destination || !view?.count}>
          Restore and verify (kind 1 only)
        </button>
      </form>
      {view?.restore && (
        <p role="status">
          Destination: {view.restore.destination} · verified{" "}
          {view.restore.verified} of {view.restore.attempted}.
        </p>
      )}
    </section>
  );
}
