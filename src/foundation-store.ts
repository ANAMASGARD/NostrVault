import { FoundationFailure } from "./foundation-contract";

// This isolated proof store is not a user vault or its production schema.
const STORE = "encrypted-records";
// ProtectedRecord JSON carries numeric ciphertext bytes; never a production format.
const LIMIT = 4 * 1024 * 1024 + 4096;
export type StoreWrite = { key: string; bytes: Uint8Array };

export async function openProofStore(name: string): Promise<IDBDatabase> {
  if (!/^nostrvault-foundation-[a-zA-Z0-9-]{1,64}$/.test(name)) {
    throw new FoundationFailure("storage");
  }
  return new Promise((resolve, reject) => {
    let blocked = false;
    const request = indexedDB.open(name, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onerror = () => reject(new FoundationFailure("storage"));
    request.onblocked = () => {
      blocked = true;
      reject(new FoundationFailure("storage"));
    };
    request.onsuccess = () => {
      const db = request.result;
      if (blocked) {
        db.close();
        return;
      }
      db.onversionchange = () => db.close();
      resolve(db);
    };
  });
}

export function writeProofBatch(
  db: IDBDatabase,
  writes: StoreWrite[],
  failAt?: number,
): Promise<void> {
  if (
    writes.length > 16 ||
    writes.some(
      ({ key, bytes }) => !key || key.length > 64 || bytes.byteLength > LIMIT,
    )
  )
    return Promise.reject(new FoundationFailure("limit"));
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE, "readwrite");
    const store = transaction.objectStore(STORE);
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(new FoundationFailure("storage"));
    transaction.onerror = () => {}; // onabort owns the final outcome.
    writes.forEach(({ key, bytes }, index) => {
      const request = store.put(bytes, key);
      if (index === failAt) request.onsuccess = () => transaction.abort();
    });
  });
}

export function readProofRecord(
  db: IDBDatabase,
  key: string,
): Promise<Uint8Array | undefined> {
  if (!key || key.length > 64)
    return Promise.reject(new FoundationFailure("limit"));
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE, "readonly");
    const request = transaction.objectStore(STORE).get(key);
    transaction.oncomplete = () => {
      const value: unknown = request.result;
      if (
        value === undefined ||
        (value instanceof Uint8Array && value.byteLength <= LIMIT)
      )
        resolve(value);
      else reject(new FoundationFailure("storage"));
    };
    transaction.onabort = () => reject(new FoundationFailure("storage"));
  });
}

export function deleteProofRecord(db: IDBDatabase, key: string): Promise<void> {
  if (!key || key.length > 64)
    return Promise.reject(new FoundationFailure("limit"));
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE, "readwrite");
    transaction.objectStore(STORE).delete(key);
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(new FoundationFailure("storage"));
  });
}

export function deleteProofStore(name: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase(name);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(new FoundationFailure("storage"));
    request.onblocked = () => reject(new FoundationFailure("storage"));
  });
}

export function equalBytes(
  actual: Uint8Array | undefined,
  expected: Uint8Array,
): boolean {
  return (
    actual !== undefined &&
    actual.length === expected.length &&
    actual.every((value, index) => value === expected[index])
  );
}

export async function checkProofStore(
  db: IDBDatabase,
  encrypted: Uint8Array,
): Promise<void> {
  await writeProofBatch(db, [{ key: "record", bytes: encrypted }]);
  if (
    !equalBytes(await readProofRecord(db, "record"), encrypted) ||
    (await readProofRecord(db, "missing")) !== undefined
  )
    throw new FoundationFailure("storage");
  const replacement = encrypted.slice().reverse();
  await writeProofBatch(db, [{ key: "record", bytes: replacement }]);
  if (!equalBytes(await readProofRecord(db, "record"), replacement))
    throw new FoundationFailure("storage");
  let aborted = false;
  try {
    await writeProofBatch(
      db,
      [
        { key: "record", bytes: encrypted },
        { key: "second", bytes: encrypted },
        { key: "third", bytes: encrypted },
      ],
      1,
    );
  } catch {
    aborted = true;
  }
  if (
    !aborted ||
    !equalBytes(await readProofRecord(db, "record"), replacement) ||
    (await readProofRecord(db, "second")) !== undefined ||
    (await readProofRecord(db, "third")) !== undefined
  )
    throw new FoundationFailure("storage");
  await deleteProofRecord(db, "record");
  if ((await readProofRecord(db, "record")) !== undefined)
    throw new FoundationFailure("storage");
  await writeProofBatch(db, [{ key: "record", bytes: encrypted }]);
}
