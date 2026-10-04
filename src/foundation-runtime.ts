import {
  parseFoundationResponse,
  MAX_EVENT,
  MAX_PAYLOAD,
  type FoundationRequest,
  type FoundationResult,
} from "./foundation-contract";
import { deleteProofStore } from "./foundation-store";

let active = false;

function runWorker(
  request: FoundationRequest,
  signal?: AbortSignal,
): Promise<FoundationResult> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(
      new URL("./foundation.worker.ts", import.meta.url),
      { type: "module" },
    );
    let settled = false;
    const finish = (error?: Error, result?: FoundationResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      signal?.removeEventListener("abort", abort);
      worker.onmessage = null;
      worker.terminate();
      if (error) reject(error);
      else if (result) resolve(result);
    };
    const abort = () => finish(new Error("Foundation request cancelled"));
    const timeout = setTimeout(
      () => finish(new Error("Foundation request timed out")),
      60000,
    );
    worker.onerror = () => finish(new Error("Foundation Worker failed"));
    worker.onmessageerror = () =>
      finish(new Error("Foundation response rejected"));
    worker.onmessage = (event: MessageEvent<unknown>) => {
      try {
        finish(
          undefined,
          parseFoundationResponse(event.data, request.requestId),
        );
      } catch (failure: unknown) {
        finish(
          failure instanceof Error
            ? failure
            : new Error("Foundation response rejected"),
        );
      }
    };
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) {
      abort();
      return;
    }
    worker.postMessage(request, [request.event.buffer, request.payload.buffer]);
  });
}

export async function proveBrowserFoundation(
  event: Uint8Array,
  payload: Uint8Array,
  signal?: AbortSignal,
): Promise<FoundationResult> {
  if (event.byteLength > MAX_EVENT || payload.byteLength > MAX_PAYLOAD)
    throw new Error("Foundation payload limit exceeded");
  if (active) throw new Error("Foundation request already active");
  active = true;
  const store = `nostrvault-foundation-${crypto.randomUUID()}`;
  try {
    const request = (phase: "prepare" | "reopen"): FoundationRequest => ({
      version: 1,
      requestId: crypto.randomUUID(),
      operation: "foundation_proof",
      eventLength: event.byteLength,
      payloadLength: payload.byteLength,
      phase,
      store,
      event: event.slice(),
      payload: payload.slice(),
    });
    await runWorker(request("prepare"), signal);
    const result = await runWorker(request("reopen"), signal);
    if (!result.storageReopened)
      throw new Error("Foundation store was not reopened");
    return result;
  } finally {
    try {
      await deleteProofStore(store);
    } finally {
      active = false;
    }
  }
}
