import { parseProbeResponse, type ProbeRequest } from "./wasm-contract";

export function probeWasm(input: number): Promise<number> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./runtime.worker.ts", import.meta.url), {
      type: "module",
    });
    const request: ProbeRequest = { requestId: crypto.randomUUID(), input };
    const finish = (error?: Error, result?: number) => {
      clearTimeout(timeout);
      worker.terminate();
      if (error) reject(error);
      else if (result !== undefined) resolve(result);
    };
    const timeout = setTimeout(
      () => finish(new Error("WASM runtime timed out")),
      10000,
    );
    worker.onerror = () => finish(new Error("WASM Worker failed"));
    worker.onmessageerror = () => finish(new Error("Invalid Worker message"));
    worker.onmessage = (event: MessageEvent<unknown>) => {
      try {
        finish(undefined, parseProbeResponse(event.data, request.requestId));
      } catch {
        finish(new Error("WASM runtime failed"));
      }
    };
    worker.postMessage(request);
  });
}
