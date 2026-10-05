import {
  parseBackup,
  type BackupAction,
  type BackupOutput,
} from "./backup-contract";
import {
  parseIdentity,
  type IdentityAction,
  type IdentityOutput,
  type Binding,
  type Effect,
} from "./identity-contract";
import { browserSigner } from "./signer-broker";
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
  private signerAbort = new AbortController();
  private active = false;
  private identityStatus: IdentityOutput | null = null;
  private signerEpoch = 0;
  private requests = new Map<
    string,
    {
      resolve: (v: unknown) => void;
      reject: (e: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  async backup(action: BackupAction): Promise<BackupOutput> {
    if (!__NATIVE_BUILD__ || this.status?.state !== "unlocked")
      throw new Error("unsupported");
    const generation = this.generation;
    const requestId = crypto.randomUUID();
    const binding = {
      vaultId: this.status.vaultId,
      token: this.status.token,
      generation: this.status.generation,
    };
    const { invoke } = await import("@tauri-apps/api/core");
    const result = parseBackup(
      await invoke<unknown>("backup_command", {
        request: JSON.stringify({ requestId, binding, action }),
      }),
    );
    if (
      generation !== this.generation ||
      result.requestId !== requestId ||
      result.binding.token !== binding.token ||
      result.binding.vaultId !== binding.vaultId ||
      result.binding.generation !== binding.generation
    )
      throw new Error("cancelled");
    return result;
  }
  private async send(request: VaultRequest): Promise<VaultStatus> {
    if (__NATIVE_BUILD__) {
      const { invoke } = await import("@tauri-apps/api/core");
      return parseStatus(
        await invoke<unknown>("vault_command", {
          request: JSON.stringify(request),
        }),
      );
    }
    return parseStatus(await this.workerSend(request));
  }
  private workerSend(request: {
    requestId: string;
    [key: string]: unknown;
  }): Promise<unknown> {
    if (__NATIVE_BUILD__) throw new Error("unsupported");
    if (!this.worker) {
      this.worker = new Worker(new URL("./vault.worker.ts", import.meta.url), {
        type: "module",
      });
      this.worker.onmessage = (event: MessageEvent<unknown>) => {
        const v = event.data;
        if (!object(v) || typeof v.requestId !== "string") return;
        const p = this.requests.get(v.requestId);
        if (!p) return;
        clearTimeout(p.timer);
        this.requests.delete(v.requestId);
        if (typeof v.error === "string") p.reject(new Error(v.error));
        else p.resolve(v.result ?? v.status);
      };
      this.worker.onerror = () => this.cancelRequests();
    }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.requests.delete(request.requestId);
        reject(new Error("timeout"));
      }, 310000);
      this.requests.set(request.requestId, { resolve, reject, timer });
      this.worker?.postMessage(request);
    });
  }
  private cancelRequests() {
    for (const p of this.requests.values()) {
      clearTimeout(p.timer);
      p.reject(new Error("cancelled"));
    }
    this.requests.clear();
  }
  async identity(action: IdentityAction): Promise<IdentityOutput> {
    if (!this.status || this.status.state !== "unlocked")
      throw new Error("locked");
    if (
      action.kind === "cancel" ||
      action.kind === "disconnect" ||
      action.kind === "grants"
    )
      this.cancelSigner();
    const binding: Binding = this.identityStatus?.binding ?? {
      vaultId: this.status.vaultId ?? "",
      token: this.status.token,
      generation: this.status.generation,
      account: null,
      signerGeneration: 0,
      consentRevision: 0,
    };
    const request = { requestId: crypto.randomUUID(), binding, action };
    const generation = this.generation;
    const value = __NATIVE_BUILD__
      ? await (
          await import("@tauri-apps/api/core")
        ).invoke<unknown>("identity_command", {
          request: JSON.stringify(request),
        })
      : await this.workerSend({
          requestId: request.requestId,
          kind: "identity",
          request,
        });
    if (generation !== this.generation) throw new Error("cancelled");
    const result = parseIdentity(value);
    if (result.requestId !== request.requestId) throw new Error("malformed");
    this.identityStatus = result;
    return result;
  }
  async signer(effect: Effect): Promise<IdentityOutput> {
    const generation = this.generation,
      epoch = this.signerEpoch;
    let value: unknown;
    try {
      const work =
        effect.adapter === "browser"
          ? browserSigner(effect, this.signerAbort.signal)
          : __NATIVE_BUILD__
            ? (await import("@tauri-apps/api/core")).invoke<unknown>(
                "identity_transport",
                { binding: effect.binding, id: effect.id },
              )
            : this.workerSend({
                requestId: crypto.randomUUID(),
                kind: "identity_transport",
                effect,
              });
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        value = await Promise.race([
          work,
          new Promise((_, reject) => {
            timer = setTimeout(
              () => reject(new Error("timeout")),
              Math.max(1, effect.deadline * 1000 - Date.now()),
            );
          }),
        ]);
      } finally {
        clearTimeout(timer);
      }
      if (object(value) && typeof value.failure === "string")
        throw new Error(value.failure);
    } catch (error: unknown) {
      if (generation !== this.generation || epoch !== this.signerEpoch)
        // Signer exceptions may contain plaintext; do not attach their cause.
        // eslint-disable-next-line preserve-caught-error
        throw new Error("cancelled");
      const code =
        error instanceof Error &&
        [
          "missing_signer",
          "unsupported",
          "denied",
          "unavailable",
          "cancelled",
          "timeout",
          "wrong_account",
          "malformed",
          "revoked",
        ].includes(error.message)
          ? error.message
          : "unavailable";
      return this.identity({ kind: "failure", id: effect.id, code });
    }
    if (generation !== this.generation || epoch !== this.signerEpoch)
      throw new Error("cancelled");
    return this.identity({ kind: "reply", id: effect.id, value });
  }

  async run(operation: VaultOperation): Promise<VaultStatus> {
    validatePasswords(operation);
    if (this.active) throw new Error("busy");
    if (operation.kind === "change_password") this.cancelSigner();
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
      if (status.state !== "unlocked") this.identityStatus = null;
      return status;
    } catch (error: unknown) {
      // Raw native errors must not retain sensitive input in an error cause.
      // eslint-disable-next-line preserve-caught-error
      throw new Error(failureCode(error));
    } finally {
      if (generation === this.generation) this.active = false;
    }
  }
  private cancelSigner() {
    this.signerEpoch++;
    this.signerAbort.abort();
    this.signerAbort = new AbortController();
  }
  async lock(): Promise<VaultStatus> {
    this.generation++;
    this.cancelSigner();
    this.identityStatus = null;
    this.cancelRequests();
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
      if (status.state !== "unlocked") this.identityStatus = null;
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
    this.active = false;
    this.generation++;
    this.cancelSigner();
    this.identityStatus = null;
    this.cancelRequests();
    this.worker?.terminate();
    this.worker = null;
    this.status = null;
  }
}
