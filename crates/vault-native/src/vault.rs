//! Production native host. All entry points share OS ownership and the same path.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};
use vault_core::vault::{
    Entropy, Error, Header, Mutation, Operation, Record, Request, Result, Session, Snapshot, State,
    Status, BATCH_LIMIT, BYTES_LIMIT, CIPHER_LIMIT,
};

pub struct OsEntropy;
impl Entropy for OsEntropy {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<()> {
        getrandom::getrandom(bytes).map_err(|_| Error::Randomness)
    }
}
fn storage(error: rusqlite::Error) -> Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DiskFull) => Error::Quota,
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            Error::Busy
        }
        _ => Error::Storage,
    }
}
pub struct Store {
    pub(crate) connection: Connection,
    _owner: File,
}
impl Store {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|_| Error::Storage)?;
        // The host selects this directory. Never unlink this file: all processes
        // must lock the same inode, including future headless JNI callers.
        let owner = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("owner.lock"))
            .map_err(|_| Error::Storage)?;
        owner.try_lock().map_err(|_| Error::Busy)?;
        let mut connection = Connection::open(directory.join("vault.sqlite")).map_err(storage)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        if version > 2 {
            return Err(Error::Unsupported);
        }
        // Refuse unsupported vault formats before changing pragmas or schemas.
        if version > 0 {
            let length: Option<u32> = connection
                .query_row(
                    "SELECT length(header) FROM metadata WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(length) = length {
                if length as usize > vault_core::vault::HEADER_LIMIT {
                    return Err(Error::Limit);
                }
                let header: Vec<u8> = connection
                    .query_row("SELECT header FROM metadata WHERE singleton=1", [], |r| {
                        r.get(0)
                    })
                    .map_err(storage)?;
                Header::parse(&header)?;
            }
        }
        connection
            .execute_batch(
                "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=MEMORY;",
            )
            .map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if version == 0 {
            tx.execute_batch("CREATE TABLE metadata (singleton INTEGER PRIMARY KEY CHECK(singleton=1), header BLOB NOT NULL, revision INTEGER NOT NULL); CREATE TABLE records (key TEXT PRIMARY KEY, version INTEGER NOT NULL, revision INTEGER NOT NULL, bytes BLOB NOT NULL, namespace TEXT NOT NULL);").map_err(storage)?;
        }
        if version < 2 {
            tx.execute_batch("CREATE TABLE staging (key TEXT PRIMARY KEY, version INTEGER NOT NULL, revision INTEGER NOT NULL, bytes BLOB NOT NULL, namespace TEXT NOT NULL); CREATE TABLE spool (id TEXT NOT NULL, namespace TEXT NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY(namespace,id)); PRAGMA user_version=2;").map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok(Self {
            connection,
            _owner: owner,
        })
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let row: Option<(Vec<u8>, u32)> = self
            .connection
            .query_row(
                "SELECT header,revision FROM metadata WHERE singleton=1 AND length(header)<=4096",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        match row {
            Some((bytes, revision)) => Ok(Snapshot {
                header: Some(Header::parse(&bytes)?),
                revision,
                setup: None,
            }),
            None => {
                let count: u32 = self
                    .connection
                    .query_row(
                        "SELECT (SELECT count(*) FROM metadata)+(SELECT count(*) FROM records)",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(storage)?;
                if count != 0 {
                    return Err(Error::Limit);
                }
                Ok(Snapshot {
                    header: None,
                    revision: 0,
                    setup: None,
                })
            }
        }
    }
    pub fn read(&self, key: &str) -> Result<Option<Record>> {
        if key.len() != 64 {
            return Err(Error::Limit);
        }
        let length: Option<u32> = self
            .connection
            .query_row(
                "SELECT length(bytes) FROM records WHERE key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if length.is_some_and(|n| n as usize > CIPHER_LIMIT) {
            return Err(Error::Limit);
        }
        let record = self
            .connection
            .query_row(
                "SELECT version,revision,bytes,namespace FROM records WHERE key=?1",
                [key],
                |r| {
                    Ok(Record {
                        namespace: r.get(3)?,
                        key: key.into(),
                        version: r.get(0)?,
                        revision: r.get(1)?,
                        bytes: r.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(storage)?;
        if let Some(record) = &record {
            record.validate()?;
        }
        Ok(record)
    }
    pub fn page(&self, namespace: &str, after: &str, limit: usize) -> Result<Vec<Record>> {
        if namespace.len() != 64 || limit == 0 || limit > BATCH_LIMIT || after.len() > 64 {
            return Err(Error::Limit);
        }
        let mut statement = self
            .connection
            .prepare("SELECT key FROM records WHERE key>?1 AND namespace=?3 ORDER BY key LIMIT ?2")
            .map_err(storage)?;
        let keys = statement
            .query_map(params![after, limit as u32, namespace], |r| {
                r.get::<_, String>(0)
            })
            .map_err(storage)?;
        let mut records = Vec::new();
        let mut bytes = 0;
        for key in keys {
            let record = self.read(&key.map_err(storage)?)?.ok_or(Error::Storage)?;
            if bytes + record.bytes.len() > BYTES_LIMIT {
                break;
            }
            bytes += record.bytes.len();
            records.push(record);
        }
        Ok(records)
    }
    /// Migration callback decrypts/re-encrypts one bounded record at a time.
    /// All writes, including staging, contain ciphertext; rollback retains old data.
    pub fn migrate_records(
        &mut self,
        expected: u32,
        mut transform: impl FnMut(&Record) -> Result<Record>,
    ) -> Result<()> {
        let next = expected.checked_add(1).ok_or(Error::Limit)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let actual: u32 = tx
            .query_row("SELECT revision FROM metadata WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .map_err(storage)?;
        if actual != expected {
            return Err(Error::Conflict);
        }
        tx.execute("DELETE FROM staging", []).map_err(storage)?;
        {
            let mut query=tx.prepare("SELECT key,version,revision,bytes,namespace,length(bytes) FROM records ORDER BY key").map_err(storage)?;
            let mut rows = query.query([]).map_err(storage)?;
            while let Some(row) = rows.next().map_err(storage)? {
                let size: u32 = row.get(5).map_err(storage)?;
                if size as usize > CIPHER_LIMIT {
                    return Err(Error::Limit);
                }
                let old = Record {
                    key: row.get(0).map_err(storage)?,
                    version: row.get(1).map_err(storage)?,
                    revision: row.get(2).map_err(storage)?,
                    bytes: row.get(3).map_err(storage)?,
                    namespace: row.get(4).map_err(storage)?,
                };
                old.validate()?;
                let new = transform(&old)?;
                new.validate()?;
                if old.key != new.key || old.namespace != new.namespace {
                    return Err(Error::Conflict);
                }
                tx.execute(
                    "INSERT INTO staging VALUES(?1,?2,?3,?4,?5)",
                    params![new.key, new.version, new.revision, new.bytes, new.namespace],
                )
                .map_err(storage)?;
            }
        }
        tx.execute_batch(
            "DELETE FROM records; INSERT INTO records SELECT * FROM staging; DELETE FROM staging;",
        )
        .map_err(storage)?;
        tx.execute("UPDATE metadata SET revision=?1 WHERE singleton=1", [next])
            .map_err(storage)?;
        tx.commit().map_err(storage)
    }
    pub fn commit(&mut self, mutation: &Mutation) -> Result<()> {
        self.commit_inner(mutation, &[], None)
    }
    pub fn delete(&mut self, expected: u32, keys: &[String]) -> Result<()> {
        self.commit_inner(
            &Mutation {
                expected_revision: expected,
                header: None,
                records: vec![],
                create: false,
            },
            keys,
            None,
        )
    }
    fn commit_inner(
        &mut self,
        mutation: &Mutation,
        deletes: &[String],
        fail_at: Option<usize>,
    ) -> Result<()> {
        if mutation.records.len() + deletes.len() > BATCH_LIMIT
            || mutation
                .records
                .iter()
                .map(|r| r.bytes.len())
                .sum::<usize>()
                > BYTES_LIMIT + 40 * BATCH_LIMIT
            || deletes.iter().any(|k| k.len() != 64)
        {
            return Err(Error::Limit);
        }
        for record in &mutation.records {
            record.validate()?;
        }
        let header = mutation.header.as_ref().map(Header::bytes).transpose()?;
        let next = mutation
            .expected_revision
            .checked_add(1)
            .ok_or(Error::Limit)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let revision: Option<u32> = tx
            .query_row("SELECT revision FROM metadata WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)?;
        if mutation.create {
            if revision.is_some() {
                return Err(Error::Exists);
            }
            tx.execute(
                "INSERT INTO metadata VALUES(1,?1,1)",
                [header.ok_or(Error::Malformed)?],
            )
            .map_err(storage)?;
        } else {
            if revision != Some(mutation.expected_revision) {
                return Err(Error::Conflict);
            }
            if let Some(header) = header {
                tx.execute(
                    "UPDATE metadata SET header=?1,revision=?2 WHERE singleton=1",
                    params![header, next],
                )
                .map_err(storage)?;
            } else {
                tx.execute("UPDATE metadata SET revision=?1 WHERE singleton=1", [next])
                    .map_err(storage)?;
            }
        }
        for (index, record) in mutation.records.iter().enumerate() {
            if fail_at == Some(index) {
                return Err(Error::Quota);
            }
            tx.execute("INSERT INTO records VALUES(?1,?2,?3,?4,?5) ON CONFLICT(key) DO UPDATE SET version=excluded.version,revision=excluded.revision,bytes=excluded.bytes,namespace=excluded.namespace", params![record.key,record.version,record.revision,record.bytes,record.namespace]).map_err(storage)?;
        }
        for key in deletes {
            tx.execute("DELETE FROM records WHERE key=?1", [key])
                .map_err(storage)?;
        }
        if fail_at == Some(mutation.records.len()) {
            return Err(Error::Quota);
        }
        tx.commit().map_err(storage)
    }
}

struct Inner {
    session: Session,
    store: Option<Store>,
}
pub struct Runtime {
    directory: PathBuf,
    epoch: AtomicU64,
    active: AtomicBool,
    inner: Mutex<Inner>,
}
impl Runtime {
    pub fn new(directory: PathBuf) -> Result<Self> {
        Ok(Self {
            directory,
            epoch: AtomicU64::new(0),
            active: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                session: Session::new(&mut OsEntropy)?,
                store: None,
            }),
        })
    }
    pub fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// Clears unlocked session state when the host process stays alive after UI teardown.
    pub fn secure_lock(&self) -> Result<Status> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let mut inner = self.inner.lock().map_err(|_| Error::Storage)?;
        inner.session.lock();
        inner.store = None;
        Ok(inner.session.status())
    }
    /// Runs on a host blocking executor, never on the UI thread.
    pub fn execute(&self, request: &Request) -> Result<Status> {
        let locking = matches!(request.operation, Operation::Lock);
        if locking {
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = if locking {
            None
        } else {
            if self.active.swap(true, Ordering::SeqCst) {
                return Err(Error::Busy);
            }
            Some(Guard(&self.active))
        };
        let epoch = self.epoch.load(Ordering::SeqCst);
        let mut inner = self.inner.lock().map_err(|_| Error::Storage)?;
        if locking {
            // Lock is always safe to apply, including a caller whose unlock reply
            // was invalidated before it learned the current session generation.
            inner.session.lock();
            inner.store = None;
            return Ok(inner.session.status());
        }
        let outcome = (|| {
            if inner.store.is_none() {
                inner.store = Some(Store::open(&self.directory)?);
            }
            let snapshot = inner.store.as_ref().ok_or(Error::Storage)?.snapshot()?;
            if matches!(request.operation, Operation::Status) {
                inner.session.observe(&snapshot)?;
            }
            let mutation = inner.session.prepare(request, &snapshot, &mut OsEntropy)?;
            if epoch != self.epoch.load(Ordering::SeqCst) {
                return Err(Error::Cancelled);
            }
            if let Some(mutation) = mutation {
                inner
                    .store
                    .as_mut()
                    .ok_or(Error::Storage)?
                    .commit(&mutation)?;
                inner.session.committed()?;
            }
            if matches!(request.operation, Operation::Unlock { .. }) {
                let key = inner.session.lookup_setup()?;
                let record = inner
                    .store
                    .as_ref()
                    .ok_or(Error::Storage)?
                    .read(&key)?
                    .ok_or(Error::Storage)?;
                inner.session.finish_unlock(&record)?;
                let index = inner.session.identity_index()?;
                let record = inner.store.as_ref().ok_or(Error::Storage)?.read(&index)?;
                if let Some(key) = inner.session.identity_pointer(record.as_ref())? {
                    let record = inner
                        .store
                        .as_ref()
                        .ok_or(Error::Storage)?
                        .read(&key)?
                        .ok_or(Error::Storage)?;
                    inner.session.identity_restore(&record)?;
                }
            }
            if epoch != self.epoch.load(Ordering::SeqCst) {
                return Err(Error::Cancelled);
            }
            Ok(inner.session.status())
        })();
        if outcome.is_err() {
            inner.session.lock();
        }
        if inner.session.status().state != State::Unlocked {
            inner.store = None;
        }
        outcome
    }
}

pub fn identity_now() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_secs())
        .map_err(|_| Error::Malformed)
}
impl Runtime {
    pub fn identity_execute(
        &self,
        request: vault_core::identity::Request,
    ) -> Result<vault_core::identity::Output> {
        let epoch = self.epoch.load(Ordering::SeqCst);
        let mut inner = self.inner.lock().map_err(|_| Error::Storage)?;
        let revision = inner
            .store
            .as_ref()
            .ok_or(Error::Locked)?
            .snapshot()?
            .revision;
        let prepared =
            inner
                .session
                .identity_prepare(request, revision, identity_now()?, &mut OsEntropy)?;
        if epoch != self.epoch.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        if let Some(mutation) = prepared.mutation {
            if let Err(error) = inner.store.as_mut().ok_or(Error::Locked)?.commit(&mutation) {
                inner.session.lock();
                inner.store = None;
                return Err(error);
            }
            inner.session.identity_committed(revision + 1);
        }
        if epoch != self.epoch.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        Ok(prepared.output)
    }
    /// Internal connection-owner API. No Tauri command exposes arbitrary event signing.
    pub fn identity_auth(
        &self,
        relay: &str,
        challenge: &str,
        connection: &str,
    ) -> Result<vault_core::identity::Effect> {
        self.inner
            .lock()
            .map_err(|_| Error::Storage)?
            .session
            .identity_auth(
                relay,
                challenge,
                connection,
                identity_now()?,
                &mut OsEntropy,
            )
    }
    pub fn identity_take_auth(&self) -> Result<Option<vault_core::identity::VerifiedAuth>> {
        self.inner
            .lock()
            .map_err(|_| Error::Storage)?
            .session
            .identity_take_auth()
    }
    pub fn identity_cancel_auth(&self) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::Storage)?
            .session
            .identity_cancel_auth();
        Ok(())
    }
    pub fn identity_effect(
        &self,
        binding: &vault_core::identity::Binding,
        id: &str,
    ) -> Result<vault_core::identity::Effect> {
        self.inner
            .lock()
            .map_err(|_| Error::Storage)?
            .session
            .identity_effect(binding, id, identity_now()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::vault;
    fn dir(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nv-m03-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn production_restart_atomicity_ownership_and_privacy() {
        let directory = dir("store");
        let mut store = Store::open(&directory).unwrap();
        assert!(matches!(Store::open(&directory), Err(Error::Busy)));
        let keys = vault::Keys::create("test password only", &mut OsEntropy).unwrap();
        let one = keys
            .seal(
                "account-a",
                "first",
                1,
                b"PRIVATE NAME AND MESSAGE",
                &mut OsEntropy,
            )
            .unwrap();
        store
            .commit(&Mutation {
                expected_revision: 0,
                header: Some(keys.header.clone()),
                records: vec![one.clone()],
                create: true,
            })
            .unwrap();
        let two = keys
            .seal(
                "account-b",
                "second",
                2,
                b"SECOND PRIVATE MARKER",
                &mut OsEntropy,
            )
            .unwrap();
        let batch = Mutation {
            expected_revision: 1,
            header: None,
            records: vec![two.clone(), one.clone()],
            create: false,
        };
        assert_eq!(store.commit_inner(&batch, &[], Some(1)), Err(Error::Quota));
        assert!(store.read(&two.key).unwrap().is_none());
        assert_eq!(store.snapshot().unwrap().revision, 1);
        store.commit(&batch).unwrap();
        assert_eq!(store.commit(&batch), Err(Error::Conflict));
        assert_eq!(store.page(&one.namespace, "", 16).unwrap().len(), 1);
        drop(store);
        let mut store = Store::open(&directory).unwrap();
        let reopened = vault::Keys::unlock(
            store.snapshot().unwrap().header.unwrap(),
            "test password only",
        )
        .unwrap();
        assert_eq!(
            &*reopened
                .open(
                    "account-a",
                    "first",
                    &store.read(&one.key).unwrap().unwrap()
                )
                .unwrap(),
            b"PRIVATE NAME AND MESSAGE"
        );
        assert!(reopened.open("account-b", "first", &one).is_err());
        let mut calls = 0;
        assert_eq!(
            store.migrate_records(2, |record| {
                calls += 1;
                if calls == 2 {
                    return Err(Error::Quota);
                }
                Ok(record.clone())
            }),
            Err(Error::Quota)
        );
        assert_eq!(store.snapshot().unwrap().revision, 2);
        assert!(store.read(&two.key).unwrap().is_some());
        store
            .migrate_records(2, |record| Ok(record.clone()))
            .unwrap();
        assert_eq!(store.snapshot().unwrap().revision, 3);
        let changed = keys
            .rewrap(
                "test password only",
                "second fixture password",
                &mut OsEntropy,
            )
            .unwrap();
        let wrapper = Mutation {
            expected_revision: 3,
            header: Some(changed),
            records: vec![],
            create: false,
        };
        assert_eq!(
            store.commit_inner(&wrapper, &[], Some(0)),
            Err(Error::Quota)
        );
        assert_eq!(store.snapshot().unwrap().header.unwrap(), keys.header);
        store.commit(&wrapper).unwrap();
        assert_eq!(store.read(&one.key).unwrap().unwrap().bytes, one.bytes);
        store.delete(4, &[two.key]).unwrap();
        assert_eq!(store.page(&one.namespace, "", 16).unwrap().len(), 1);
        drop(store);
        let bytes = std::fs::read(directory.join("vault.sqlite")).unwrap();
        for marker in [
            b"PRIVATE NAME AND MESSAGE".as_slice(),
            b"test password only",
            b"account-a",
        ] {
            assert!(!bytes.windows(marker.len()).any(|w| w == marker));
            assert!(marker.windows(marker.len()).any(|w| w == marker));
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    fn request(status: &Status, operation: Operation) -> Request {
        Request {
            version: 1,
            request_id: "native-test".into(),
            token: status.token.clone(),
            generation: status.generation,
            vault_id: status.vault_id.clone(),
            operation,
        }
    }
    #[test]
    fn identity_persists_encrypted_and_disconnect_survives_reopen() {
        use vault_core::identity::{Action, Adapter, Binding, Request as IdentityRequest};
        let dir = std::env::temp_dir().join(format!("nv-identity-{}", std::process::id()));
        let host = Runtime::new(dir.clone()).unwrap();
        let initial = host.inner.lock().unwrap().session.status();
        let created = host
            .execute(&request(
                &initial,
                Operation::Create {
                    password: "test identity password".into(),
                    confirmation: "test identity password".into(),
                },
            ))
            .unwrap();
        let binding = Binding {
            vault_id: created.vault_id.clone().unwrap(),
            token: created.token.clone(),
            generation: created.generation,
            account: None,
            signer_generation: 0,
            consent_revision: 0,
        };
        let output = host
            .identity_execute(IdentityRequest {
                request_id: "connect".into(),
                binding,
                action: Action::Connect {
                    adapter: Adapter::Browser,
                    remember: true,
                    pairing: None,
                    relays: vec![],
                    package: None,
                },
            })
            .unwrap();
        let account = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let output=host.identity_execute(IdentityRequest{request_id:"reply".into(),binding:output.binding,action:Action::Reply{id:output.effect.unwrap().id,value:serde_json::json!({"account":account,"package":null,"capabilities":{"publicKey":true,"nip04":false,"nip44":false,"relayAuth":false}})}}).unwrap();
        host.identity_execute(IdentityRequest {
            request_id: "confirm".into(),
            binding: output.binding,
            action: Action::Confirm {
                account: account.into(),
            },
        })
        .unwrap();
        drop(host);
        let bytes = std::fs::read(dir.join("vault.sqlite")).unwrap();
        assert!(!bytes
            .windows(account.len())
            .any(|w| w == account.as_bytes()));
        let host = Runtime::new(dir.clone()).unwrap();
        let initial = host.inner.lock().unwrap().session.status();
        let initial = host.execute(&request(&initial, Operation::Status)).unwrap();
        host.execute(&request(
            &initial,
            Operation::Unlock {
                password: "test identity password".into(),
            },
        ))
        .unwrap();
        let binding = host
            .inner
            .lock()
            .unwrap()
            .session
            .identity_binding()
            .unwrap();
        let output = host
            .identity_execute(IdentityRequest {
                request_id: "status".into(),
                binding,
                action: Action::Status,
            })
            .unwrap();
        assert_eq!(output.view.account.as_deref(), Some(account));
        assert_eq!(output.view.state, "disconnected");
        assert!(output.effect.is_none());
        host.identity_execute(IdentityRequest {
            request_id: "disconnect".into(),
            binding: output.binding,
            action: Action::Disconnect,
        })
        .unwrap();
        drop(host);
        let host = Runtime::new(dir.clone()).unwrap();
        let initial = host.inner.lock().unwrap().session.status();
        let initial = host.execute(&request(&initial, Operation::Status)).unwrap();
        host.execute(&request(
            &initial,
            Operation::Unlock {
                password: "test identity password".into(),
            },
        ))
        .unwrap();
        let binding = host
            .inner
            .lock()
            .unwrap()
            .session
            .identity_binding()
            .unwrap();
        assert!(binding.account.is_none());
        drop(host);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn actual_runtime_reopen_password_change_and_stale_generation() {
        let dir = std::env::temp_dir().join(format!("nv-runtime-{}", std::process::id()));
        let host = Runtime::new(dir.clone()).unwrap();
        let initial = host.inner.lock().unwrap().session.status();
        let initial = host.execute(&request(&initial, Operation::Status)).unwrap();
        let unlocked = host
            .execute(&request(
                &initial,
                Operation::Create {
                    password: "native test password".into(),
                    confirmation: "native test password".into(),
                },
            ))
            .unwrap();
        assert_eq!(unlocked.state, State::Unlocked);
        let stale = request(
            &unlocked,
            Operation::SaveSetup {
                setup: Default::default(),
                revision: unlocked.revision,
            },
        );
        let locked = host.execute(&request(&unlocked, Operation::Lock)).unwrap();
        assert_eq!(host.execute(&stale).unwrap_err(), Error::Cancelled);
        drop(host);
        let host = Runtime::new(dir.clone()).unwrap();
        let mut status = host.execute(&request(&locked, Operation::Status)).unwrap();
        assert_eq!(status.state, State::Locked);
        status = host
            .execute(&request(
                &status,
                Operation::Unlock {
                    password: "native test password".into(),
                },
            ))
            .unwrap();
        status = host
            .execute(&request(
                &status,
                Operation::ChangePassword {
                    current: "native test password".into(),
                    password: "changed native password".into(),
                    confirmation: "changed native password".into(),
                },
            ))
            .unwrap();
        assert_eq!(status.state, State::Locked);
        drop(host);
        let host = Runtime::new(dir.clone()).unwrap();
        status = host.execute(&request(&status, Operation::Status)).unwrap();
        assert_eq!(
            host.execute(&request(
                &status,
                Operation::Unlock {
                    password: "native test password".into()
                }
            ))
            .unwrap_err(),
            Error::Authentication
        );
        status = host.execute(&request(&status, Operation::Status)).unwrap();
        status = host
            .execute(&request(
                &status,
                Operation::Unlock {
                    password: "changed native password".into(),
                },
            ))
            .unwrap();
        assert!(status.setup.is_some());
        drop(host);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn secure_lock_clears_unlocked_session_without_ipc() {
        let dir = std::env::temp_dir().join(format!("nv-secure-lock-{}", std::process::id()));
        let host = Runtime::new(dir.clone()).unwrap();
        let initial = host.inner.lock().unwrap().session.status();
        let status = host
            .execute(&request(
                &initial,
                Operation::Create {
                    password: "native test password".into(),
                    confirmation: "native test password".into(),
                },
            ))
            .unwrap();
        assert_eq!(status.state, State::Unlocked);
        let locked = host.secure_lock().unwrap();
        assert_eq!(locked.state, State::Locked);
        drop(host);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn future_schema_refused_without_modification_and_structural_upgrade() {
        let dir = std::env::temp_dir().join(format!("nv-schema-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("vault.sqlite")).unwrap();
        conn.execute_batch("CREATE TABLE metadata(singleton INTEGER PRIMARY KEY,header BLOB,revision INTEGER);CREATE TABLE records(key TEXT PRIMARY KEY,version INTEGER,revision INTEGER,bytes BLOB,namespace TEXT);PRAGMA user_version=1;").unwrap();
        let future = serde_json::json!({"format":99,"wrapper":1,"vaultId":"00".repeat(16),"memoryKib":65536,"passes":3,"lanes":4,"revision":1,"salt":vec![0;16],"nonce":vec![0;24],"wrapped":vec![0;48]});
        conn.execute(
            "INSERT INTO metadata VALUES(1,?1,1)",
            [serde_json::to_vec(&future).unwrap()],
        )
        .unwrap();
        drop(conn);
        let before = std::fs::read(dir.join("vault.sqlite")).unwrap();
        assert!(matches!(Store::open(&dir), Err(Error::Unsupported)));
        assert_eq!(before, std::fs::read(dir.join("vault.sqlite")).unwrap());
        let conn = Connection::open(dir.join("vault.sqlite")).unwrap();
        conn.execute("DELETE FROM metadata", []).unwrap();
        drop(conn);
        let store = Store::open(&dir).unwrap();
        let version: u32 = store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 2);
        store
            .connection
            .execute_batch("PRAGMA user_version=99;")
            .unwrap();
        drop(store);
        let before = std::fs::read(dir.join("vault.sqlite")).unwrap();
        assert!(matches!(Store::open(&dir), Err(Error::Unsupported)));
        assert_eq!(before, std::fs::read(dir.join("vault.sqlite")).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// Bounded backup work never holds the vault mutex while awaiting a relay.
pub(crate) struct BackupLease {
    pub data: Option<vault_core::backup::Backup>,
    pub account: Option<String>,
    pub revision: u32,
    pub epoch: u64,
    pub identity: vault_core::identity::Binding,
}
impl Runtime {
    pub(crate) fn backup_context(
        &self,
        binding: &vault_core::backup::Binding,
    ) -> Result<BackupLease> {
        let inner = self.inner.lock().map_err(|_| Error::Storage)?;
        inner.session.backup_validate(binding)?;
        let store = inner.store.as_ref().ok_or(Error::Locked)?;
        let record = store.read(&inner.session.backup_lookup()?)?;
        Ok(BackupLease {
            data: record
                .as_ref()
                .map(|r| inner.session.backup_open(r))
                .transpose()?,
            account: inner.session.backup_account().ok(),
            revision: store.snapshot()?.revision,
            epoch: self.epoch.load(Ordering::SeqCst),
            identity: inner.session.identity_binding()?,
        })
    }
    pub(crate) fn backup_check(
        &self,
        binding: &vault_core::backup::Binding,
        lease: &BackupLease,
    ) -> Result<()> {
        let inner = self.inner.lock().map_err(|_| Error::Storage)?;
        inner.session.backup_validate(binding)?;
        if lease.epoch != self.epoch.load(Ordering::SeqCst)
            || lease.identity != inner.session.identity_binding()?
        {
            return Err(Error::Cancelled);
        }
        if inner.session.status().revision != lease.revision {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    pub(crate) fn backup_commit(
        &self,
        binding: &vault_core::backup::Binding,
        lease: &BackupLease,
        data: &vault_core::backup::Backup,
    ) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::Storage)?;
        inner.session.backup_validate(binding)?;
        if lease.epoch != self.epoch.load(Ordering::SeqCst)
            || lease.identity != inner.session.identity_binding()?
        {
            return Err(Error::Cancelled);
        }
        let mutation = inner
            .session
            .backup_seal(data, lease.revision, &mut OsEntropy)?;
        inner
            .store
            .as_mut()
            .ok_or(Error::Locked)?
            .commit(&mutation)?;
        inner.session.identity_committed(lease.revision + 1);
        Ok(())
    }
}
