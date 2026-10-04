import { useState } from "react";

import { checkRuntime } from "./runtime";
import "./App.css";

function App() {
  const [status, setStatus] = useState("Not checked");
  const [checking, setChecking] = useState(false);

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
      <p className="eyebrow">Development build · Milestone 01</p>
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
      </section>
    </main>
  );
}

export default App;
