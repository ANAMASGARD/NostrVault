import { parseIdentity, type Effect } from "./identity-contract";
import { remoteSocket } from "./identity-socket";
import init, { VaultSession } from "./generated/vault_wasm";
import wasmUrl from "./generated/vault_wasm_bg.wasm?url";
import { failureCode, object, parseStatus } from "./vault-contract";
import {
  commitVault,
  openVaultStore,
  parseMutation,
  readRecord,
  readOptionalRecord,
  readSnapshot,
} from "./vault-store";
let session: VaultSession | undefined;
let db: IDBDatabase | undefined;
let release: (() => void) | undefined;
let busy = false;
let signerSocket: AbortController | undefined;
let identityEffect: Effect | null = null;
async function ownership(): Promise<void> {
  if (release) return;
  if (!navigator.locks) throw new Error("unsupported");
  await new Promise<void>((resolve, reject) => {
    void navigator.locks
      .request(
        "nostrvault-vault-owner",
        { mode: "exclusive", ifAvailable: true },
        async (lock) => {
          if (!lock) {
            reject(new Error("busy"));
            return;
          }
          await new Promise<void>((done) => {
            release = done;
            resolve();
          });
        },
      )
      .catch(reject);
  });
}
function close() {
  signerSocket?.abort();
  identityEffect = null;
  db?.close();
  db = undefined;
  release?.();
  release = undefined;
}
addEventListener("message", (event: MessageEvent<unknown>) => {
  const request = event.data;
  if (
    object(request) &&
    typeof request.requestId === "string" &&
    request.kind === "identity_transport"
  ) {
    const effect = identityEffect;
    if (!effect || !object(request.effect) || request.effect.id !== effect.id) {
      postMessage({ requestId: request.requestId, error: "cancelled" });
      return;
    }
    signerSocket?.abort();
    const controller = new AbortController();
    signerSocket = controller;
    void remoteSocket(effect, controller.signal).then(
      (result) => postMessage({ requestId: request.requestId, result }),
      () => postMessage({ requestId: request.requestId, error: "unavailable" }),
    );
    return;
  }
  if (
    object(request) &&
    typeof request.requestId === "string" &&
    request.kind === "identity"
  ) {
    if (busy || !session || !db) {
      postMessage({
        requestId: request.requestId,
        error: busy ? "busy" : "locked",
      });
      return;
    }
    busy = true;
    void (async () => {
      try {
        if (!session || !db) throw new Error("locked");
        const snapshot = await readSnapshot(db);
        const prepared: unknown = JSON.parse(
          session.identity_prepare(
            JSON.stringify(request.request),
            snapshot.revision,
            Math.floor(Date.now() / 1000),
          ),
        );
        if (!object(prepared)) throw new Error("malformed");
        const result = parseIdentity(prepared.output);
        const mutation = parseMutation(prepared.mutation);
        if (mutation) {
          try {
            await commitVault(db, mutation);
            session.identity_committed(snapshot.revision + 1);
          } catch (error) {
            session.lock();
            close();
            throw error;
          }
        }
        identityEffect = result.effect;
        if (!identityEffect) signerSocket?.abort();
        postMessage({ requestId: request.requestId, result });
      } catch (error: unknown) {
        postMessage({
          requestId: request.requestId,
          error: failureCode(error),
        });
      } finally {
        busy = false;
      }
    })();
    return;
  }
  if (
    !object(request) ||
    typeof request.requestId !== "string" ||
    !object(request.operation) ||
    typeof request.operation.kind !== "string"
  )
    return;
  const kind = request.operation.kind;
  if (busy) {
    postMessage({ requestId: request.requestId, error: "busy" });
    return;
  }
  busy = true;
  void (async () => {
    try {
      await ownership();
      if (!session) {
        await init({ module_or_path: wasmUrl });
        session = new VaultSession();
      }
      db ??= await openVaultStore();
      const snapshot = await readSnapshot(db);
      const mutation = parseMutation(
        JSON.parse(
          session.prepare(JSON.stringify(request), JSON.stringify(snapshot)),
        ) as unknown,
      );
      if (mutation) {
        await commitVault(db, mutation);
        session.committed();
      }
      if (kind === "unlock") {
        session.finish_unlock(
          JSON.stringify(await readRecord(db, session.lookup_setup())),
        );
        const pointer: unknown = JSON.parse(
          session.identity_pointer(
            JSON.stringify(
              await readOptionalRecord(db, session.identity_index()),
            ),
          ),
        );
        if (typeof pointer === "string")
          session.identity_restore(
            JSON.stringify(await readRecord(db, pointer)),
          );
      }
      const status = parseStatus(JSON.parse(session.status()) as unknown);
      if (status.state !== "unlocked") close();
      postMessage({ requestId: request.requestId, status });
    } catch (error: unknown) {
      session?.lock();
      close();
      postMessage({ requestId: request.requestId, error: failureCode(error) });
    } finally {
      busy = false;
    }
  })();
});
