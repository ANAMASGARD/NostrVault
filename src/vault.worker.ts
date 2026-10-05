import init, { VaultSession } from "./generated/vault_wasm";
import wasmUrl from "./generated/vault_wasm_bg.wasm?url";
import { failureCode, object, parseStatus } from "./vault-contract";
import {
  commitVault,
  openVaultStore,
  parseMutation,
  readRecord,
  readSnapshot,
} from "./vault-store";
let session: VaultSession | undefined;
let db: IDBDatabase | undefined;
let release: (() => void) | undefined;
let busy = false;
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
  db?.close();
  db = undefined;
  release?.();
  release = undefined;
}
addEventListener("message", (event: MessageEvent<unknown>) => {
  const request = event.data;
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
      if (kind === "unlock")
        session.finish_unlock(
          JSON.stringify(await readRecord(db, session.lookup_setup())),
        );
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
