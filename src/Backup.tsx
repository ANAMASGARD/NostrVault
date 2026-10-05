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
              "Saved public notes could not be opened. Lock and unlock to retry.",
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
          "Operation did not complete. Saved notes remain available. Check the relay, confirm your account for backup, or retry after unlock. A failed restore may have sent events; its verification is unknown. Saturated responses and authentication-required relays are unsupported in this slice.",
        );
    } finally {
      running.current = false;
      if (live.current) setBusy(false);
    }
  }
  if (!linux) return null;
  return (
    <section aria-labelledby="backup-title">
      <h3 id="backup-title">Collect and recover public notes</h3>
      <p>
        Milestone 05 · Linux · manual collection from approved relays. Public
        history is implemented; legacy DMs and gift wraps require a later
        commit. Up to 256 notes; no completeness guarantee.
      </p>
      <fieldset>
        <legend>Lookup relays (max 3, for metadata only)</legend>
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
            void run({
              kind: "discover",
              lookupRelays,
              approved: true,
            });
          }}
        >
          Discover history relay hints
        </button>
        {view?.suggestions?.length ? (
          <ul>
            {view.suggestions.map((relay) => (
              <li key={relay}>
                {relay}{" "}
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => setSource(relay)}
                >
                  Use as source
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </fieldset>
      <label>
        Collection profile
        <select
          value={profile}
          onChange={(e) =>
            setProfile(
              e.target.value as
                | "public_history"
                | "legacy_direct_messages"
                | "gift_wraps",
            )
          }
          disabled={busy}
        >
          <option value="public_history">Public notes (kind 1)</option>
          <option value="legacy_direct_messages" disabled>
            Legacy DMs (NIP-04) — not in this commit
          </option>
          <option value="gift_wraps" disabled>
            Gift wraps (NIP-17) — not in this commit
          </option>
        </select>
      </label>
      {error && <p role="alert">{error}</p>}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run({
            kind: "capture",
            relay: source,
            approved: true,
            profile,
          });
        }}
      >
        <label>
          Source relay
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
          Connect and confirm your account above first. Pressing Back up now
          approves contacting this relay for your public notes and deletion
          requests.
        </p>
        <button disabled={busy || !source}>Back up now</button>
      </form>
      <p role="status">
        {busy
          ? "Working… You can lock the vault to cancel further work."
          : view?.outcome === "source_unavailable"
            ? "Source relay: OFFLINE / unavailable at the last check. Your saved notes remain local."
            : view?.outcome === "source_reachable"
              ? "Source relay: reachable at the last check."
              : "Saved notes can be read without the source relay or signer."}
      </p>
      <p>
        Backed up and available offline:{" "}
        <strong>{view?.count ?? 0} public notes</strong>
      </p>
      {view?.capturedAt ? (
        <p>
          Last capture: {new Date(view.capturedAt * 1000).toLocaleString()}.
          Relay response ended; this does not prove all account history was
          found.
        </p>
      ) : null}
      <p>
        Observed deletion targets: {view?.suppressed ?? 0}. Excluded inputs:{" "}
        {view?.excluded ?? 0}. Expired bodies are removed on access. Unseen
        deletions remain a collection gap.
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
        Read saved notes offline
      </button>
      <ol aria-label="Saved public notes">
        {view?.notes.map((note) => (
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
        Previous notes
      </button>
      <button
        disabled={busy || offset + 16 >= (view?.count ?? 0)}
        onClick={() => void run({ kind: "read", offset: offset + 16 })}
      >
        Next notes
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
          I approve publishing eligible original public notes to this
          destination. This does not change my advertised relays. Eligibility
          uses observed deletion evidence; unseen deletions cannot be inferred.
        </label>
        <button disabled={busy || !approved || !destination || !view?.count}>
          Restore and independently verify
        </button>
      </form>
      {view?.restore && (
        <p role="status">
          Destination: {view.restore.destination} · checked{" "}
          {new Date(view.restore.checkedAt * 1000).toLocaleString()}.
          Transmitted: {view.restore.attempted}; acknowledged:{" "}
          {view.restore.acknowledged}; rejected: {view.restore.rejected};
          independently returned and validated:{" "}
          <strong>
            {view.restore.verified} of {view.restore.attempted}
          </strong>
          .{" "}
          {view.restore.verificationComplete
            ? "Fresh destination read completed."
            : "Verification incomplete; remaining availability is unknown."}{" "}
          This proves observed availability, not permanent storage.
        </p>
      )}
    </section>
  );
}
