use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use vault_core::{ErrorCode, FoundationError, Result, MAX_BATCH, MAX_PAYLOAD};

fn storage_error(_: rusqlite::Error) -> FoundationError {
    FoundationError::new(ErrorCode::Storage)
}
pub struct Store(Connection);
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path).map_err(storage_error)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;
        if version > 1 {
            return Err(FoundationError::new(ErrorCode::Storage));
        }
        connection.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS proof_records (key TEXT PRIMARY KEY, value BLOB NOT NULL); PRAGMA user_version=1;").map_err(storage_error)?;
        Ok(Self(connection))
    }
    pub fn read(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if key.is_empty() || key.len() > 64 {
            return Err(FoundationError::new(ErrorCode::Limit));
        }
        let length: Option<i64> = self
            .0
            .query_row(
                "SELECT length(value) FROM proof_records WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        if length.is_some_and(|size| size > (4 * MAX_PAYLOAD + 4096) as i64) {
            return Err(FoundationError::new(ErrorCode::Limit));
        }
        self.0
            .query_row(
                "SELECT value FROM proof_records WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)
    }
    pub fn write_batch(&mut self, records: &[(&str, &[u8])]) -> Result<()> {
        self.write_batch_inner(records, None)
    }
    fn write_batch_inner(
        &mut self,
        records: &[(&str, &[u8])],
        fail_at: Option<usize>,
    ) -> Result<()> {
        if records.len() > MAX_BATCH
            || records.iter().any(|(key, value)| {
                key.is_empty() || key.len() > 64 || value.len() > 4 * MAX_PAYLOAD + 4096
            })
        {
            return Err(FoundationError::new(ErrorCode::Limit));
        }
        let transaction = self.0.transaction().map_err(storage_error)?;
        for (index, (key, value)) in records.iter().enumerate() {
            if fail_at == Some(index) {
                return Err(FoundationError::new(ErrorCode::Storage));
            }
            transaction.execute("INSERT INTO proof_records(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(storage_error)?;
        }
        transaction.commit().map_err(storage_error)
    }
    pub fn delete(&self, key: &str) -> Result<()> {
        if key.is_empty() || key.len() > 64 {
            return Err(FoundationError::new(ErrorCode::Limit));
        }
        self.0
            .execute("DELETE FROM proof_records WHERE key=?1", [key])
            .map_err(storage_error)?;
        Ok(())
    }
}

/// Exercises transaction rollback with ciphertext in the isolated proof store.
pub fn conformance(path: &Path, ciphertext: &[u8]) -> Result<()> {
    let mut store = Store::open(path)?;
    for key in ["proof-first", "proof-second", "proof-third"] {
        store.delete(key)?;
    }
    store.write_batch(&[("proof-first", ciphertext)])?;
    let mut replacement = ciphertext.to_vec();
    replacement.reverse();
    if store
        .write_batch_inner(
            &[
                ("proof-first", &replacement),
                ("proof-second", ciphertext),
                ("proof-third", ciphertext),
            ],
            Some(1),
        )
        .is_ok()
    {
        return Err(FoundationError::new(ErrorCode::Storage));
    }
    if store.read("proof-first")?.as_deref() != Some(ciphertext)
        || store.read("proof-second")?.is_some()
        || store.read("proof-third")?.is_some()
    {
        return Err(FoundationError::new(ErrorCode::Storage));
    }
    store.write_batch(&[("proof-first", &replacement)])?;
    drop(store);
    let store = Store::open(path)?;
    if store.read("proof-first")?.as_deref() != Some(replacement.as_slice()) {
        return Err(FoundationError::new(ErrorCode::Storage));
    }
    store.delete("proof-first")?;
    if store.read("proof-first")?.is_some() {
        return Err(FoundationError::new(ErrorCode::Storage));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_storage_survives_connection_replacement() {
        let directory =
            std::env::temp_dir().join(format!("nostrvault-storage-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("proof.sqlite");
        let mut store = Store::open(&path).unwrap();
        store.write_batch(&[("old", &[0, 255, 1])]).unwrap();
        assert_eq!(store.read("absent").unwrap(), None);
        assert!(store
            .write_batch_inner(
                &[
                    ("old", b"replaced"),
                    ("second", b"two"),
                    ("third", b"three")
                ],
                Some(1)
            )
            .is_err());
        assert_eq!(store.read("old").unwrap(), Some(vec![0, 255, 1]));
        assert_eq!(store.read("second").unwrap(), None);
        assert_eq!(store.read("third").unwrap(), None);
        store.write_batch(&[("old", b"new")]).unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.read("old").unwrap(), Some(b"new".to_vec()));
        store.delete("old").unwrap();
        assert_eq!(store.read("old").unwrap(), None);
        drop(store);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
