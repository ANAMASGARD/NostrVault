import {
  failureCode,
  object,
  parseStatus,
  validatePasswords,
  type VaultOperation,
  type VaultRequest,
  type VaultStatus,
} from "./vault-contract";
export class VaultRuntime {
  private worker: Worker | null = null;
  private status: VaultStatus | null = null;
  private generation = 0;
  private pending: ((error: Error) => void) | null = null;
  private active = false;
  private async send(request: VaultRequest): Promise<VaultStatus> {
    if (__NATIVE_BUILD__) {
      const { invoke } = await import("@tauri-apps/api/core");
      return parseStatus(
        await invoke<unknown>("vault_command", {
          request: JSON.stringify(request),
        }),
      );
    }
    this.worker ??= new Worker(new URL("./vault.worker.ts", import.meta.url), {
      type: "module",
    });
    const worker = this.worker;
    return new Promise((resolve, reject) => {
      const finish = (error?: Error, status?: VaultStatus) => {
        clearTimeout(timer);
        this.pending = null;
        worker.onmessage = null;
        worker.onerror = null;
        if (error) reject(error);
        else if (status) resolve(status);
      };
      const timer = setTimeout(() => {
        worker.terminate();
        if (this.worker === worker) this.worker = null;
        finish(new Error("cancelled"));
      }, 60000);
      this.pending = (error) => finish(error);
      worker.onerror = () => finish(new Error("storage"));
      worker.onmessage = (event: MessageEvent<unknown>) => {
        try {
          const value = event.data;
          if (!object(value) || value.requestId !== request.requestId)
            throw new Error("malformed");
          if (typeof value.error === "string") finish(new Error(value.error));
          else finish(undefined, parseStatus(value.status));
        } catch {
          finish(new Error("malformed"));
        }
      };
      worker.postMessage(request);
    });
  }
  async run(operation: VaultOperation): Promise<VaultStatus> {
    validatePasswords(operation);
    if (this.active) throw new Error("busy");
    this.active = true;
    const generation = this.generation;
    try {
      const status = await this.send({
        version: 1,
        requestId: crypto.randomUUID(),
        token: this.status?.token ?? "",
        generation: this.status?.generation ?? 0,
        vaultId: this.status?.vaultId ?? null,
        operation,
      });
      if (generation !== this.generation) throw new Error("cancelled");
      this.status = status;
      return status;
    } catch (error: unknown) {
      // Raw native errors must not retain sensitive input in an error cause.
      // eslint-disable-next-line preserve-caught-error
      throw new Error(failureCode(error));
    } finally {
      if (generation === this.generation) this.active = false;
    }
  }
  async lock(): Promise<VaultStatus> {
    this.generation++;
    this.pending?.(new Error("cancelled"));
    this.worker?.terminate();
    this.worker = null;
    this.active = false;
    if (__NATIVE_BUILD__) {
      const status = await this.send({
        version: 1,
        requestId: crypto.randomUUID(),
        token: this.status?.token ?? "",
        generation: this.status?.generation ?? 0,
        vaultId: this.status?.vaultId ?? null,
        operation: { kind: "lock" },
      });
      this.status = status;
      return status;
    }
    // Worker termination releases Web Locks asynchronously. Wait for that
    // release before asking the replacement Worker to inspect committed state.
    if (navigator.locks) {
      await navigator.locks.request(
        "nostrvault-vault-owner",
        { signal: AbortSignal.timeout(5000) },
        () => {},
      );
    }
    this.status = null;
    return this.run({ kind: "status" });
  }
  dispose(): void {
    this.generation++;
    this.pending?.(new Error("cancelled"));
    this.worker?.terminate();
    this.worker = null;
    this.status = null;
  }
}
