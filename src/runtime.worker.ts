import wasmUrl from "./generated/runtime_probe.wasm?url";
import { isProbeRequest, type ProbeResponse } from "./wasm-contract";

function isProbeExport(value: unknown): value is (input: number) => unknown {
  return typeof value === "function";
}

addEventListener("message", (event: MessageEvent<unknown>) => {
  const request = event.data;
  if (!isProbeRequest(request)) return;
  void (async () => {
    let response: ProbeResponse;
    try {
      const fetched = await fetch(wasmUrl);
      if (!fetched.ok) throw new Error("WASM fetch failed");
      const { instance } = await WebAssembly.instantiate(
        await fetched.arrayBuffer(),
      );
      const probe = instance.exports.runtime_probe;
      if (!isProbeExport(probe)) throw new Error("WASM export missing");
      const result: unknown = probe(request.input);
      if (typeof result !== "number" || !Number.isInteger(result)) {
        throw new Error("Invalid WASM result");
      }
      // The WebAssembly JS ABI represents i32 values as signed numbers.
      response = {
        requestId: request.requestId,
        type: "result",
        result: result >>> 0,
      };
    } catch {
      response = { requestId: request.requestId, type: "error" };
    }
    postMessage(response);
  })();
});
