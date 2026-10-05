import { afterEach, expect, it, vi } from "vitest";
import { VaultRuntime } from "./vault-runtime";
const workers: StubWorker[] = [];
class StubWorker {
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
  onerror: (() => void) | null = null;
  request: { requestId: string } | null = null;
  constructor() {
    workers.push(this);
  }
  postMessage(request: { requestId: string }) {
    this.request = request;
  }
  terminate() {}
}
afterEach(() => {
  vi.unstubAllGlobals();
  workers.length = 0;
});
it("a StrictMode cleanup cancels the old request without blocking the replacement runtime", async () => {
  vi.stubGlobal("Worker", StubWorker);
  const host = new VaultRuntime();
  const first = host.run({ kind: "status" });
  const rejected = expect(first).rejects.toThrow();
  host.dispose();
  const second = host.run({ kind: "status" });
  expect(workers).toHaveLength(2);
  // The disposed Worker cannot satisfy the replacement request.
  const previous = workers[0];
  previous.onmessage?.({
    data: { requestId: previous.request?.requestId, status: {} },
  } as MessageEvent<unknown>);
  host.dispose();
  await rejected;
  await expect(second).rejects.toThrow();
});
