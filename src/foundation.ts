import fixtureEvent from "../crates/vault-core/fixtures/event.json?raw";
import fixturePayload from "../crates/vault-core/fixtures/payload.zip?url";
import fixtureMetadata from "./foundation-fixture.json";
import { parseFoundationResponse } from "./foundation-contract";

export async function checkFoundation(signal?: AbortSignal): Promise<string> {
  const requestId = crypto.randomUUID();

  if (__NATIVE_BUILD__) {
    const { invoke } = await import("@tauri-apps/api/core");
    signal?.throwIfAborted();
    const pending = invoke<unknown>("foundation_proof", {
      request: JSON.stringify({
        version: 1,
        requestId,
        operation: "foundation_proof",
        eventLength: new TextEncoder().encode(fixtureEvent).length,
        payloadLength: fixtureMetadata.payloadBytes,
      }),
    });
    const response = await new Promise<unknown>((resolve, reject) => {
      const abort = () => {
        signal?.removeEventListener("abort", abort);
        reject(new Error("Foundation request cancelled"));
      };
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) abort();
      void pending
        .then(resolve, reject)
        .finally(() => signal?.removeEventListener("abort", abort));
    });
    signal?.throwIfAborted();
    const result = parseFoundationResponse(response, requestId);
    if (!result.storageReopened) throw new Error("Foundation storage failed");
    return `Rust foundation ready · ${result.payloadBytes} fixture bytes · encrypted storage reopened`;
  }
  const fetched = await fetch(fixturePayload, { signal });
  if (!fetched.ok) throw new Error("Foundation fixture unavailable");
  const payload = new Uint8Array(await fetched.arrayBuffer());
  const { proveBrowserFoundation } = await import("./foundation-runtime");
  const result = await proveBrowserFoundation(
    new TextEncoder().encode(fixtureEvent),
    payload,
    signal,
  );
  return `Rust/WASM foundation ready · ${result.payloadBytes} fixture bytes · encrypted storage reopened`;
}
