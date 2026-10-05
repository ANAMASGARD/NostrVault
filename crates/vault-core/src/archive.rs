//! Versioned portable snapshot envelope. Uses the existing authenticated
//! record cipher. A failed import does not replace the previous snapshot.
use crate::vault::{Error, Result};
use crate::{open_record, protect_record, ProtectedRecord, RandomMaterial};
use serde::{Deserialize, Serialize};

pub const ARCHIVE_VERSION: u32 = 1;
pub const MAX_ENTRIES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveManifest {
    pub version: u32,
    pub snapshot_id: String,
    pub account: String,
    pub event_count: usize,
    pub include_conversations: bool,
}

pub fn archive_filename(date: &str, snapshot_id: &str) -> Result<String> {
    validate_entry_name(date)?;
    validate_entry_name(snapshot_id)?;
    Ok(format!("nostrvault-{date}-{snapshot_id}.nvarchive"))
}

pub fn validate_entry_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 128
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
    {
        return Err(Error::Malformed);
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::Malformed);
    }
    Ok(())
}

pub fn validate_manifest(manifest: &ArchiveManifest) -> Result<()> {
    if manifest.version != ARCHIVE_VERSION {
        return Err(Error::Unsupported);
    }
    validate_entry_name(&manifest.snapshot_id)?;
    if manifest.account.len() != 64 || !manifest.account.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Malformed);
    }
    if manifest.event_count > 256 {
        return Err(Error::Limit);
    }
    Ok(())
}

pub fn seal(
    password: &[u8],
    manifest: &ArchiveManifest,
    body: &[u8],
    random: &RandomMaterial,
) -> Result<ProtectedRecord> {
    validate_manifest(manifest)?;
    if body.len() > crate::MAX_PAYLOAD {
        return Err(Error::Limit);
    }
    let payload = serde_json::to_vec(&(manifest, body)).map_err(|_| Error::Malformed)?;
    protect_record(password, &payload, &manifest.account, "archive", random)
        .map_err(|_| Error::Crypto)
}

pub fn unseal(
    password: &[u8],
    record: &ProtectedRecord,
    account: &str,
) -> Result<(ArchiveManifest, Vec<u8>)> {
    let plain =
        open_record(password, record, account, "archive").map_err(|_| Error::Authentication)?;
    let (manifest, body): (ArchiveManifest, Vec<u8>) =
        serde_json::from_slice(&plain).map_err(|_| Error::Malformed)?;
    validate_manifest(&manifest)?;
    if manifest.account != account {
        return Err(Error::Authentication);
    }
    Ok((manifest, body))
}

/// Keep `previous` unless the candidate unseals cleanly.
pub fn import_replace(
    previous: &[u8],
    password: &[u8],
    candidate: &ProtectedRecord,
    account: &str,
) -> Result<Vec<u8>> {
    match unseal(password, candidate, account) {
        Ok(_) => Ok(serde_json::to_vec(candidate).map_err(|_| Error::Storage)?),
        Err(_) => {
            let _ = previous;
            Err(Error::Authentication)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ArchiveManifest {
        ArchiveManifest {
            version: 1,
            snapshot_id: "snap1".into(),
            account: "ab".repeat(32),
            event_count: 1,
            include_conversations: true,
        }
    }

    fn random() -> RandomMaterial {
        RandomMaterial {
            salt: [7; 16],
            data_key: [8; 32],
            wrap_nonce: [9; 24],
            record_nonce: [10; 24],
        }
    }

    #[test]
    fn round_trip_and_wrong_password() {
        let sealed = seal(b"correct-password", &manifest(), b"events", &random()).unwrap();
        let (opened, body) = unseal(b"correct-password", &sealed, &manifest().account).unwrap();
        assert_eq!(opened.snapshot_id, "snap1");
        assert_eq!(body, b"events");
        assert!(unseal(b"wrong-password!!", &sealed, &manifest().account).is_err());
    }

    #[test]
    fn truncated_and_traversal_fail() {
        let mut sealed = seal(b"correct-password", &manifest(), b"events", &random()).unwrap();
        sealed.ciphertext.pop();
        assert!(unseal(b"correct-password", &sealed, &manifest().account).is_err());
        assert!(validate_entry_name("../etc").is_err());
        assert!(validate_entry_name("/tmp/x").is_err());
        let kept = b"previous".to_vec();
        assert!(import_replace(&kept, b"wrong-password!!", &sealed, &manifest().account).is_err());
    }
}
