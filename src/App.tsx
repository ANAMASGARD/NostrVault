import { useEffect, useRef, useState } from "react";

import { checkFoundation } from "./foundation";
import { checkRuntime } from "./runtime";
import "./App.css";

function App() {
  const [status, setStatus] = useState("Not checked");
  const [checking, setChecking] = useState(false);

  const [foundation, setFoundation] = useState("Foundation not checked");
  const controller = useRef<AbortController | null>(null);
  useEffect(() => () => controller.current?.abort(), []);

  async function prove() {
    const current = new AbortController();
    controller.current = current;
    setFoundation("Checking encrypted fixture storage…");
    try {
      setFoundation(await checkFoundation(current.signal));
    } catch {
      setFoundation("Foundation check failed or cancelled. Try again.");
    } finally {
      controller.current = null;
    }
  }

  async function check() {
    setChecking(true);
    setStatus("Checking runtime…");
    try {
      setStatus(await checkRuntime());
    } catch {
      setStatus("Runtime check failed. Try again.");
    } finally {
      setChecking(false);
    }
  }

  return (
    <main>
      <p className="eyebrow">Development build · Milestone 02</p>
      <h1>NostrVault</h1>
      <p>Your Nostr history should survive your relay.</p>
      <section aria-labelledby="foundation-title">
        <h2 id="foundation-title">Platform foundation</h2>
        <p>
          Backup, signer connection, and offline conversations are not available
          in this build. No account or relay is contacted.
        </p>
        <button disabled={checking} onClick={() => void check()}>
          Check runtime
        </button>
        <p role="status">{status}</p>
        <button
          disabled={foundation === "Checking encrypted fixture storage…"}
          onClick={() => void prove()}
        >
          Check shared engine
        </button>
        <button
          disabled={foundation !== "Checking encrypted fixture storage…"}
          onClick={() => controller.current?.abort()}
        >
          Cancel engine check
        </button>
        <p aria-live="polite" data-testid="foundation-result">
          {foundation}
        </p>
        <p>
          Fixture proof only. No user vault is created. Proof passwords and
          crypto parameters are not production defaults.
        </p>
      </section>
    </main>
  );
}

export default App;
