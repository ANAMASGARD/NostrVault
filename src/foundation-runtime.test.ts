import { afterEach, expect, it, vi } from "vitest";
import { proveBrowserFoundation } from "./foundation-runtime";
import type { FoundationRequest } from "./foundation-contract";

vi.mock("./foundation-store", () => ({
  deleteProofStore: vi.fn().mockResolvedValue(undefined),
}));
class TestWorker {
  static instances: TestWorker[] = [];
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
  onerror: (() => void) | null = null;
  onmessageerror: (() => void) | null = null;
  request?: FoundationRequest;
  terminated = false;
  constructor() {
    TestWorker.instances.push(this);
  }
  postMessage(request: FoundationRequest) {
    this.request = request;
  }
  terminate() {
    this.terminated = true;
  }
}
afterEach(() => {
  vi.unstubAllGlobals();
  TestWorker.instances = [];
});
it("rejects concurrent work, cancels its Worker, and ignores late delivery", async () => {
  vi.stubGlobal("Worker", TestWorker);
  const controller = new AbortController();
  const pending = proveBrowserFoundation(
    new Uint8Array([1]),
    new Uint8Array([2]),
    controller.signal,
  );
  await expect(
    proveBrowserFoundation(new Uint8Array([1]), new Uint8Array([2])),
  ).rejects.toThrow("already active");
  const worker = TestWorker.instances[0];
  const lateHandler = worker.onmessage;
  controller.abort();
  await expect(pending).rejects.toThrow("cancelled");
  expect(worker.terminated).toBe(true);
  expect(worker.onmessage).toBeNull();
  lateHandler?.(
    new MessageEvent("message", {
      data: {
        version: 1,
        requestId: worker.request?.requestId,
        error: null,
        result: {
          eventId: "a".repeat(64),
          payloadBytes: 1,
          cryptoVerified: true,
          storageReopened: true,
        },
      },
    }),
  );
  const replacement = proveBrowserFoundation(
    new Uint8Array([1]),
    new Uint8Array([2]),
  );
  const newer = TestWorker.instances[1];
  newer.onmessage?.(
    new MessageEvent("message", {
      data: {
        version: 1,
        requestId: worker.request?.requestId,
        error: null,
        result: {},
      },
    }),
  );
  await expect(replacement).rejects.toThrow("response rejected");
  expect(newer.terminated).toBe(true);
  const success = proveBrowserFoundation(
    new Uint8Array([1]),
    new Uint8Array([2]),
  );
  const reply = (worker: TestWorker, reopened: boolean) =>
    worker.onmessage?.(
      new MessageEvent("message", {
        data: {
          version: 1,
          requestId: worker.request?.requestId,
          error: null,
          result: {
            eventId: "a".repeat(64),
            payloadBytes: 1,
            cryptoVerified: true,
            storageReopened: reopened,
          },
        },
      }),
    );
  reply(TestWorker.instances[2], false);
  await Promise.resolve();
  reply(TestWorker.instances[3], true);
  await expect(success).resolves.toMatchObject({
    storageReopened: true,
    cryptoVerified: true,
  });
  expect(TestWorker.instances.every((worker) => worker.terminated)).toBe(true);
});
