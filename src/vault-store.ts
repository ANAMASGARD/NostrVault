import { integer, object } from "./vault-contract";
export type StoredRecord = {
  namespace: string;
  key: string;
  version: number;
  revision: number;
  bytes: number[];
};
export type Snapshot = { header: unknown; revision: number; setup: null };
export type Mutation = {
  expectedRevision: number;
  header: unknown;
  records: StoredRecord[];
  create: boolean;
};
export function parseRecord(value: unknown): StoredRecord {
  if (
    !object(value) ||
    typeof value.namespace !== "string" ||
    !/^[a-f0-9]{64}$/.test(value.namespace) ||
    typeof value.key !== "string" ||
    !/^[a-f0-9]{64}$/.test(value.key) ||
    value.version !== 1 ||
    !integer(value.revision) ||
    value.revision === 0 ||
    !Array.isArray(value.bytes) ||
    value.bytes.length < 40 ||
    value.bytes.length > 1048616 ||
    !value.bytes.every((b: unknown) => integer(b) && b <= 255)
  )
    throw new Error("malformed");
  return {
    namespace: value.namespace,
    key: value.key,
    version: 1,
    revision: value.revision,
    bytes: value.bytes as number[],
  };
}
export function parseMutation(value: unknown): Mutation | null {
  if (value === null) return null;
  if (
    !object(value) ||
    !integer(value.expectedRevision) ||
    typeof value.create !== "boolean" ||
    !Array.isArray(value.records) ||
    value.records.length > 16
  )
    throw new Error("malformed");
  const records = value.records.map(parseRecord);
  if (records.reduce((sum, r) => sum + r.bytes.length, 0) > 4194304 + 640)
    throw new Error("limit");
  if (value.header !== null && JSON.stringify(value.header).length > 4096)
    throw new Error("limit");
  return {
    expectedRevision: value.expectedRevision,
    header: value.header,
    records,
    create: value.create,
  };
}
function storageError(error: DOMException | null): Error {
  return new Error(error?.name === "QuotaExceededError" ? "quota" : "storage");
}
export function openVaultStore(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let abandoned = false;
    let upgradeError: string | undefined;
    const request = indexedDB.open("nostrvault-vault", 2);
    request.onupgradeneeded = (event) => {
      const db = request.result;
      for (const name of ["metadata", "records", "staging"])
        if (!db.objectStoreNames.contains(name)) db.createObjectStore(name);
      const upgrade = request.transaction;
      if (event.oldVersion > 0 && upgrade) {
        const previous = upgrade.objectStore("metadata").get("vault");
        previous.onsuccess = () => {
          const value: unknown = previous.result;
          if (
            value !== undefined &&
            (!object(value) ||
              !object(value.header) ||
              value.header.format !== 1 ||
              value.header.wrapper !== 1)
          ) {
            upgradeError = "unsupported";
            upgrade.abort();
          }
        };
      }
      const records = request.transaction?.objectStore("records");
      if (records && !records.indexNames.contains("namespace-key"))
        records.createIndex("namespace-key", ["namespace", "key"], {
          unique: true,
        });
    };
    request.onblocked = () => {
      abandoned = true;
      reject(new Error("busy"));
    };
    request.onerror = () =>
      reject(
        new Error(
          upgradeError ??
            (request.error?.name === "VersionError"
              ? "unsupported"
              : failure(request.error)),
        ),
      );
    request.onsuccess = () => {
      const db = request.result;
      if (abandoned) {
        db.close();
        return;
      }
      db.onversionchange = () => db.close();
      resolve(db);
    };
  });
}
function failure(error: DOMException | null): string {
  return error?.name === "QuotaExceededError" ? "quota" : "storage";
}
export function readSnapshot(db: IDBDatabase): Promise<Snapshot> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(["metadata", "records"], "readonly");
    const count = tx.objectStore("records").count();
    const request = tx.objectStore("metadata").get("vault");
    tx.onabort = () => reject(storageError(tx.error));
    tx.oncomplete = () => {
      const value: unknown = request.result;
      if (value === undefined && count.result === 0)
        resolve({ header: null, revision: 0, setup: null });
      else if (
        object(value) &&
        integer(value.revision) &&
        value.header !== undefined &&
        JSON.stringify(value.header).length <= 4096
      )
        resolve({
          header: value.header,
          revision: value.revision,
          setup: null,
        });
      else reject(new Error("malformed"));
    };
  });
}
export function readRecord(
  db: IDBDatabase,
  key: string,
): Promise<StoredRecord> {
  if (!/^[a-f0-9]{64}$/.test(key)) return Promise.reject(new Error("limit"));
  return new Promise((resolve, reject) => {
    const tx = db.transaction("records", "readonly");
    const request = tx.objectStore("records").get(key);
    tx.onabort = () => reject(storageError(tx.error));
    tx.oncomplete = () => {
      try {
        resolve(parseRecord(request.result));
      } catch {
        reject(new Error("storage"));
      }
    };
  });
}
export function commitVault(
  db: IDBDatabase,
  mutation: Mutation,
  failAt?: number,
  deletes: string[] = [],
): Promise<void> {
  if (
    deletes.length + mutation.records.length > 16 ||
    deletes.some((key) => !/^[a-f0-9]{64}$/.test(key))
  )
    return Promise.reject(new Error("limit"));
  return new Promise((resolve, reject) => {
    const tx = db.transaction(["metadata", "records"], "readwrite", {
      durability: "strict",
    });
    let code = "storage";
    tx.oncomplete = () => resolve();
    tx.onabort = () =>
      reject(
        new Error(tx.error?.name === "QuotaExceededError" ? "quota" : code),
      );
    tx.onerror = () => {};
    const metadata = tx.objectStore("metadata");
    const request = metadata.get("vault");
    request.onsuccess = () => {
      const current: unknown = request.result;
      if (
        mutation.create
          ? current !== undefined
          : !object(current) || current.revision !== mutation.expectedRevision
      ) {
        code = mutation.create ? "exists" : "conflict";
        tx.abort();
        return;
      }
      const header =
        mutation.header ?? (object(current) ? current.header : null);
      if (header === null || mutation.expectedRevision === 0xffffffff) {
        code = "limit";
        tx.abort();
        return;
      }
      metadata.put(
        { header, revision: mutation.expectedRevision + 1 },
        "vault",
      );
      deletes.forEach((key) => tx.objectStore("records").delete(key));
      mutation.records.forEach((record, index) => {
        const write = tx.objectStore("records").put(record, record.key);
        if (index === failAt)
          write.onsuccess = () => {
            code = "quota";
            tx.abort();
          };
      });
    };
  });
}

export function pageRecords(
  db: IDBDatabase,
  namespace: string,
  after: string,
  limit: number,
): Promise<StoredRecord[]> {
  if (
    !/^[a-f0-9]{64}$/.test(namespace) ||
    after.length > 64 ||
    !Number.isInteger(limit) ||
    limit < 1 ||
    limit > 16
  )
    return Promise.reject(new Error("limit"));
  return new Promise((resolve, reject) => {
    const tx = db.transaction("records");
    const records: StoredRecord[] = [];
    let bytes = 0;
    let failure: Error | undefined;
    const cursor = tx
      .objectStore("records")
      .index("namespace-key")
      .openCursor(
        IDBKeyRange.bound([namespace, after], [namespace, "g"], true, false),
      );
    cursor.onsuccess = () => {
      const row = cursor.result;
      if (!row) return;
      try {
        const record = parseRecord(row.value);
        if (bytes + record.bytes.length > 4194304 || records.length >= limit)
          return;
        bytes += record.bytes.length;
        records.push(record);
        row.continue();
      } catch {
        failure = new Error("storage");
        tx.abort();
      }
    };
    tx.oncomplete = () => resolve(records);
    tx.onabort = () => reject(failure ?? storageError(tx.error));
  });
}
export function deleteRecords(
  db: IDBDatabase,
  expectedRevision: number,
  keys: string[],
): Promise<void> {
  return commitVault(
    db,
    { expectedRevision, header: null, records: [], create: false },
    undefined,
    keys,
  );
}
// Payload migration stages already-encrypted bounded pages. All old records stay
// active until a complete same-key replacement set is atomically finalized.
export function stageMigration(
  db: IDBDatabase,
  expectedRevision: number,
  records: StoredRecord[],
  start: boolean,
): Promise<void> {
  const validated = parseMutation({
    expectedRevision,
    header: null,
    records,
    create: false,
  });
  if (!validated) return Promise.reject(new Error("malformed"));
  return new Promise((resolve, reject) => {
    const tx = db.transaction(["metadata", "records", "staging"], "readwrite", {
      durability: "strict",
    });
    let code = "storage";
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(new Error(code));
    const metadata = tx.objectStore("metadata");
    const request = metadata.get("vault");
    request.onsuccess = () => {
      const current: unknown = request.result;
      if (!object(current) || current.revision !== expectedRevision) {
        code = "conflict";
        tx.abort();
        return;
      }
      if (start) {
        tx.objectStore("staging").clear();
        metadata.put(expectedRevision, "migration");
      }
      const marker = metadata.get("migration");
      marker.onsuccess = () => {
        if (marker.result !== expectedRevision) {
          code = "conflict";
          tx.abort();
          return;
        }
        for (const record of validated.records) {
          const original = tx.objectStore("records").get(record.key);
          original.onsuccess = () => {
            const value: unknown = original.result;
            if (!object(value) || value.namespace !== record.namespace) {
              code = "conflict";
              tx.abort();
            } else tx.objectStore("staging").put(record, record.key);
          };
        }
      };
    };
  });
}
export function finalizeMigration(
  db: IDBDatabase,
  expectedRevision: number,
  abort = false,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(["metadata", "records", "staging"], "readwrite", {
      durability: "strict",
    });
    let code = "storage";
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(new Error(code));
    const metadata = tx.objectStore("metadata");
    const request = metadata.get("vault");
    request.onsuccess = () => {
      const current: unknown = request.result;
      if (
        !object(current) ||
        current.revision !== expectedRevision ||
        expectedRevision === 0xffffffff
      ) {
        code = "conflict";
        tx.abort();
        return;
      }
      const marker = metadata.get("migration");
      marker.onsuccess = () => {
        if (marker.result !== expectedRevision) {
          code = "conflict";
          tx.abort();
          return;
        }
        const original = tx.objectStore("records").count();
        original.onsuccess = () => {
          const count = tx.objectStore("staging").count();
          count.onsuccess = () => {
            if (count.result !== original.result) {
              code = "conflict";
              tx.abort();
              return;
            }
            tx.objectStore("records").clear();
            const cursor = tx.objectStore("staging").openCursor();
            cursor.onsuccess = () => {
              const row = cursor.result;
              if (row) {
                tx.objectStore("records").put(row.value, row.key);
                row.continue();
              } else if (abort) {
                code = "quota";
                tx.abort();
              } else {
                metadata.put(
                  { ...current, revision: expectedRevision + 1 },
                  "vault",
                );
                metadata.delete("migration");
                tx.objectStore("staging").clear();
              }
            };
          };
        };
      };
    };
  });
}
