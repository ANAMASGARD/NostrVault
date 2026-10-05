import { useEffect, useRef, useState } from "react";
import { Backup } from "./Backup";
import { Chats } from "./Chats";
import { Identity } from "./Identity";
import { failureCode, messages, type VaultStatus } from "./vault-contract";
import { VaultRuntime } from "./vault-runtime";

export function Vault() {
  const runtime = useRef<VaultRuntime | null>(null);
  const [host] = useState(() => new VaultRuntime());
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [changing, setChanging] = useState(false);
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [current, setCurrent] = useState("");
  const [show, setShow] = useState(false);
  const [ack, setAck] = useState(false);
  const epoch = useRef(0);
  const submitting = useRef(false);
  const passwordField = useRef<HTMLInputElement>(null);
  function clear() {
    setPassword("");
    setConfirmation("");
    setCurrent("");
    setShow(false);
  }
  useEffect(() => {
    runtime.current = host;
    let live = true;
    void host.run({ kind: "status" }).then(
      (value) => {
        if (live) setStatus(value);
      },
      (failure: unknown) => {
        if (live)
          setError(
            messages[failureCode(failure)] ??
              "Vault storage could not be opened. Retry when it is available.",
          );
      },
    );
    return () => {
      live = false;
      host.dispose();
      runtime.current = null;
    };
  }, [host]);
  useEffect(() => {
    if (status?.state === "locked" || status?.state === "absent")
      passwordField.current?.focus();
  }, [status?.state]);
  async function retry() {
    if (!runtime.current) return;
    setError("");
    try {
      setStatus(await runtime.current.run({ kind: "status" }));
    } catch (failure: unknown) {
      setError(
        messages[failureCode(failure)] ?? "Vault storage could not be opened.",
      );
    }
  }
  async function lock() {
    if (!runtime.current) return;
    epoch.current++;
    submitting.current = true;
    clear();
    setChanging(false);
    setStatus(null);
    setBusy(true);
    setError("");
    try {
      setStatus(await runtime.current.lock());
    } catch (failure: unknown) {
      setError(
        messages[failureCode(failure)] ??
          "Vault locked. Retry to check saved state.",
      );
    } finally {
      submitting.current = false;
      setBusy(false);
    }
  }
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!runtime.current || submitting.current || !status) return;
    submitting.current = true;
    const generation = epoch.current;
    setBusy(true);
    setError("");
    const operation = changing
      ? { kind: "change_password" as const, current, password, confirmation }
      : status.state === "absent"
        ? { kind: "create" as const, password, confirmation }
        : { kind: "unlock" as const, password };
    clear();
    try {
      const next = await runtime.current.run(operation);
      if (generation === epoch.current) {
        setStatus(next);
        setChanging(false);
      }
    } catch (failure: unknown) {
      if (generation === epoch.current) {
        setError(
          messages[failureCode(failure)] ??
            "The operation failed. Your existing committed vault has been preserved.",
        );
        try {
          setStatus(await runtime.current.run({ kind: "status" }));
        } catch {
          setStatus(null);
        }
      }
    } finally {
      if (generation === epoch.current) {
        submitting.current = false;
        setBusy(false);
      }
    }
  }
  const creating = status?.state === "absent";
  const form = creating || status?.state === "locked" || changing;
  return (
    <section aria-labelledby="vault-title">
      <h2 id="vault-title">Protect your saved backup</h2>
      <p>
        Vault operations stay local. Signer connections require your explicit
        action.
      </p>
      {error && <p role="alert">{error}</p>}
      {!status && !busy && (
        <button onClick={() => void retry()}>Retry storage</button>
      )}
      {busy && <p role="status">Working…</p>}
      {form && (
        <form onSubmit={(event) => void submit(event)}>
          <p>
            {creating
              ? "Create an encrypted local vault, then connect your account."
              : changing
                ? "Confirm your current password to protect the same vault with a new password."
                : "Unlock your saved local vault."}
          </p>
          {changing && (
            <label>
              Current password
              <input
                type={show ? "text" : "password"}
                value={current}
                autoComplete="current-password"
                onChange={(event) => setCurrent(event.target.value)}
                required
                disabled={busy}
              />
            </label>
          )}
          <label>
            {changing ? "New password" : "Password"}
            <input
              ref={passwordField}
              type={show ? "text" : "password"}
              value={password}
              autoComplete={
                creating || changing ? "new-password" : "current-password"
              }
              onChange={(event) => setPassword(event.target.value)}
              required
              disabled={busy}
            />
          </label>
          {(creating || changing) && (
            <label>
              Confirm password
              <input
                type={show ? "text" : "password"}
                value={confirmation}
                autoComplete="new-password"
                onChange={(event) => setConfirmation(event.target.value)}
                required
                disabled={busy}
              />
            </label>
          )}
          <label className="check">
            <input
              type="checkbox"
              checked={show}
              onChange={(event) => setShow(event.target.checked)}
            />
            Show passwords
          </label>
          {(creating || changing) && (
            <p>
              Use at least 12 characters. A long, unique passphrase is
              recommended. Passwords are preserved exactly, including spaces.
            </p>
          )}
          {creating && (
            <label className="check">
              <input
                type="checkbox"
                checked={ack}
                onChange={(event) => setAck(event.target.checked)}
                required
              />
              I understand that losing this password loses access to my local
              vault. A Nostr signer cannot reset it.
            </label>
          )}
          <button disabled={busy || (creating && !ack)} type="submit">
            {creating
              ? "Create vault"
              : changing
                ? "Change password"
                : "Unlock"}
          </button>
          {changing && (
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                clear();
                setChanging(false);
              }}
            >
              Back
            </button>
          )}
        </form>
      )}
      {status?.state === "unlocked" && !changing && (
        <>
          <p>Your local vault is ready.</p>
          <p role="status">Setup saved securely on this device.</p>
          <Identity host={host} />
          <Chats messages={[]} />
          {__NATIVE_BUILD__ && <Backup host={host} />}
          <button
            onClick={() => {
              clear();
              setChanging(true);
            }}
          >
            Change password
          </button>
        </>
      )}
      {(status?.state === "unlocked" || busy) && (
        <button onClick={() => void lock()}>
          {busy ? "Cancel and lock" : "Lock now"}
        </button>
      )}
      <p>
        Vault passwords are separate from Nostr account access. Future exports
        will default to your vault password; old archives and external copies
        keep their original password. Changing this password cannot revoke those
        copies.
      </p>
    </section>
  );
}
