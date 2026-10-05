import { useEffect, useRef, useState } from "react";
import {
  type Adapter,
  type IdentityAction,
  type IdentityOutput,
} from "./identity-contract";
import { VaultRuntime } from "./vault-runtime";
const reasons: Record<string, string> = {
  missing_signer:
    "No compatible signer is available. Install or unlock your signer, then retry.",
  unsupported: "This signer does not provide that operation.",
  denied: "Approval was declined. Nothing will retry automatically.",
  unavailable: "Your signer needs attention. Unlock it and retry when ready.",
  cancelled: "Signer operation cancelled.",
  timeout:
    "The approval request timed out. Finish or dismiss the old signer prompt before retrying.",
  wrong_account:
    "The signer returned a different account or an invalid capability proof. Reconnect to the confirmed account.",
  malformed: "The signer response could not be validated.",
  revoked:
    "Your signer permission was revoked. Review it in your signer before retrying.",
};
export function Identity({ host }: { host: VaultRuntime }) {
  const [output, setOutput] = useState<IdentityOutput | null>(null);
  const [adapter, setAdapter] = useState<Adapter>(
    __NATIVE_BUILD__ ? "remote" : "browser",
  );
  const [packages, setPackages] = useState<string[]>([]),
    [selected, setSelected] = useState("");
  const [remember, setRemember] = useState(false),
    [pairing, setPairing] = useState(""),
    [relays, setRelays] = useState("");
  const [authRelays, setAuthRelays] = useState("");
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const active = useRef(false),
    epoch = useRef(0);
  useEffect(() => {
    let live = true;
    const lifecycle = epoch;
    void host.identity({ kind: "status" }).then(
      (v) => {
        if (live) setOutput(v);
      },
      () => {
        if (live) setError("Unlock the vault again to inspect account state.");
      },
    );
    if (__NATIVE_BUILD__)
      void import("@tauri-apps/api/core").then(async ({ invoke }) => {
        const platform = await invoke<{ platform: string }>("runtime_info");
        if (platform.platform === "android") {
          const values = await invoke<string[]>("identity_packages");
          if (live) {
            setAdapter("android");
            setPackages(values);
            setSelected(values[0] ?? "");
          }
        }
      });
    return () => {
      live = false;
      lifecycle.current++;
    };
  }, [host]);
  async function run(action: IdentityAction) {
    const cancelling = action.kind === "cancel" || action.kind === "disconnect";
    if (active.current && !cancelling) return;
    if (cancelling) epoch.current++;
    const generation = epoch.current;
    active.current = true;
    setBusy(true);
    setError("");
    try {
      let next = await host.identity(action);
      while (generation === epoch.current) {
        setOutput(next);
        if (!next.effect) break;
        next = await host.signer(next.effect);
      }
    } catch {
      if (generation === epoch.current)
        setError(
          "The operation could not be completed. Cancel or unlock again to reconcile saved state.",
        );
    } finally {
      if (generation === epoch.current) {
        active.current = false;
        setBusy(false);
      }
    }
  }
  const view = output?.view;
  const waiting = view?.state === "waiting_for_approval";
  const connected = view?.state === "connected";
  return (
    <section aria-labelledby="identity-title">
      <h3 id="identity-title">Connect Nostr account</h3>
      <p>
        Your signer keeps your identity secret. Backup collection is not
        implemented yet.
      </p>
      {error && <p role="alert">{error}</p>}
      {view?.failure && (
        <p role="alert">
          Needs attention:{" "}
          {reasons[view.failure] ?? "Review your signer and retry."}
        </p>
      )}
      {waiting && <p role="status">Waiting for approval</p>}
      {view?.account && (
        <p>
          Account: <code className="identity-key">{view.account}</code>
        </p>
      )}
      {view?.state === "awaiting_confirmation" && view.account && (
        <>
          <p>Confirm this is the account you intend to connect.</p>
          <button
            disabled={busy}
            onClick={() =>
              void run({ kind: "confirm", account: view.account ?? "" })
            }
          >
            Confirm account
          </button>
        </>
      )}
      {!view?.account && !waiting && (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            const value = pairing;
            setPairing("");
            void run({
              kind: "connect",
              adapter,
              remember,
              pairing: value || null,
              relays: relays.split(/\s+/).filter(Boolean),
              package: adapter === "android" ? selected : null,
            });
          }}
        >
          {adapter !== "android" && (
            <label>
              Connection method
              <select
                value={adapter}
                onChange={(e) =>
                  setAdapter(e.target.value === "remote" ? "remote" : "browser")
                }
                disabled={busy}
              >
                {!__NATIVE_BUILD__ && (
                  <option value="browser">Browser signer</option>
                )}
                <option value="remote">Remote signer</option>
              </select>
            </label>
          )}
          {adapter === "android" && (
            <label>
              Installed signer
              <select
                value={selected}
                onChange={(e) => setSelected(e.target.value)}
              >
                <option value="">Choose a signer</option>
                {packages.map((p) => (
                  <option key={p}>{p}</option>
                ))}
              </select>
            </label>
          )}
          {adapter === "remote" && (
            <>
              <label>
                Connection link from your signer (optional)
                <input
                  value={pairing}
                  onChange={(e) => setPairing(e.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                />
              </label>
              <label>
                Approved signer relay URLs, separated by spaces
                <input
                  required
                  value={relays}
                  onChange={(e) => setRelays(e.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                />
              </label>
              <p>
                Only these relays will carry your encrypted signer requests.
                Leave the connection link empty to generate one for your signer.
              </p>
            </>
          )}
          <label className="check">
            <input
              type="checkbox"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
            />
            Remember{" "}
            {adapter === "remote"
              ? "this encrypted remote connection"
              : adapter === "android"
                ? "this account and signer app"
                : "this account and local consent choices"}
          </label>
          <p>
            {adapter === "remote"
              ? "Remote session credentials are saved only encrypted inside this vault when enabled."
              : "Your signer owns its remembered permissions. NostrVault does not save or control its login."}
          </p>
          <button
            disabled={busy || !output || (adapter === "android" && !selected)}
          >
            Connect
          </button>
        </form>
      )}
      {view?.pairing && (
        <label>
          Connection link to enter in your signer
          <textarea
            readOnly
            value={view.pairing}
            aria-label="Connection link to enter in your signer"
          />
        </label>
      )}
      {connected && (
        <>
          <p role="status">Account connected</p>
          <dl>
            <dt>Public identity</dt>
            <dd>Available</dd>
            <dt>Legacy private-message decryption</dt>
            <dd>{capability(view.capabilities.nip04)}</dd>
            <dt>Private-message decryption</dt>
            <dd>{capability(view.capabilities.nip44)}</dd>
            <dt>Relay authentication signing</dt>
            <dd>{capability(view.capabilities.relayAuth)}</dd>
          </dl>
          {view.adapter === "browser" && (
            <p>
              Private-chat decryption through browser extensions is unavailable
              in this MVP. Use a verified native signer. Public identity
              connection remains available.
            </p>
          )}
          <label className="check">
            <input
              type="checkbox"
              checked={view.grants.readable}
              disabled={busy || view.adapter === "browser"}
              onChange={(e) =>
                void run({
                  kind: "grants",
                  grants: { ...view.grants, readable: e.target.checked },
                })
              }
            />
            Readable private chats: allow decryption and encrypted local
            retention when backup becomes available.
          </label>
          {view.grants.readable && (
            <>
              <p>
                Checking support uses a disposable encrypted challenge and may
                open a signer approval prompt. It reads no message history.
              </p>
              <button
                disabled={busy || view.capabilities.nip44 === false}
                onClick={() =>
                  void run({ kind: "probe", method: "nip44_decrypt" })
                }
              >
                Approve private-message capability check
              </button>
              <button
                disabled={busy || view.capabilities.nip04 === false}
                onClick={() =>
                  void run({ kind: "probe", method: "nip04_decrypt" })
                }
              >
                Approve legacy capability check
              </button>
            </>
          )}
          <details>
            <summary>Advanced</summary>
            <p>
              Relay authentication permission is limited to the exact URLs
              below. It does not publish history or start a connection.
            </p>
            <p>
              Approved relay authentication:{" "}
              {view.grants.relayAuth.join(", ") || "None"}
            </p>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void run({
                  kind: "grants",
                  grants: {
                    ...view.grants,
                    relayAuth: authRelays.split(/\s+/).filter(Boolean),
                  },
                });
              }}
            >
              <label>
                Relay URLs allowed to request account authentication
                <input
                  value={authRelays}
                  onChange={(event) => setAuthRelays(event.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                />
              </label>
              <button disabled={busy}>Save authentication scope</button>
            </form>
            <p>
              Adapter: {view.adapter}. Replication, attachments, background
              execution, routing changes and history collection remain
              unavailable. No publication permission is requested.
            </p>
          </details>
        </>
      )}
      {view?.account &&
        !connected &&
        view.state !== "awaiting_confirmation" &&
        !waiting && (
          <button
            disabled={busy}
            onClick={() => void run({ kind: "reconnect" })}
          >
            Retry connection
          </button>
        )}
      {waiting && (
        <button onClick={() => void run({ kind: "cancel" })}>
          Cancel signer request
        </button>
      )}
      {(view?.account || view?.adapter) && (
        <button onClick={() => void run({ kind: "disconnect" })}>
          Disconnect and remove local signer access
        </button>
      )}
      <p>
        Disconnect removes NostrVault’s remembered connection. Revoke external
        remembered permissions in your signer separately.
      </p>
    </section>
  );
}
function capability(value: boolean | null) {
  return value === null
    ? "Not yet verified"
    : value
      ? "Available; approval may still be required"
      : "Unavailable";
}
