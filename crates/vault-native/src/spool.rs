//! Sealed ingress primitives only: no collector, scheduler, or UI enable switch.
use crate::vault::{OsEntropy, Store};
use age::secrecy::ExposeSecret;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use vault_core::vault::{Error, Keys, Record, Result, RECORD_LIMIT};
const MAX_SEALED: usize = RECORD_LIMIT + 65536;
const QUOTA: usize = 64 * 1024 * 1024;

// This grant is deliberately transient. M03 does not persist locked-capture
// configuration or allow an untrusted caller to manufacture one through IPC.
pub struct Grant {
    vault: String,
    account: String,
    key_id: String,
    recipient: age::x25519::Recipient,
    enabled: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    version: u32,
    vault: String,
    account: String,
    key_id: String,
    id: String,
    body: Vec<u8>,
}
impl Drop for Batch {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.account.zeroize();
        self.body.zeroize();
        self.id.zeroize();
    }
}
fn failure(_: rusqlite::Error) -> Error {
    Error::Storage
}
impl Grant {
    pub fn create(keys: &Keys, account: &str, enabled: bool) -> Result<(Self, Record)> {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let key_id = keys.lookup(account, "ingress-key")?;
        let secret = identity.to_string();
        let record = keys.seal(
            account,
            "ingress-key",
            1,
            secret.expose_secret().as_bytes(),
            &mut OsEntropy,
        )?;
        Ok((
            Self {
                vault: keys.header.vault_id.clone(),
                account: account.into(),
                key_id,
                recipient,
                enabled,
            },
            record,
        ))
    }
    pub fn seal(&self, id: &str, body: &[u8]) -> Result<Vec<u8>> {
        if !self.enabled {
            return Err(Error::Locked);
        }
        if id.is_empty() || id.len() > 128 || body.len() > RECORD_LIMIT {
            return Err(Error::Limit);
        }
        let plain = zeroize::Zeroizing::new(
            serde_json::to_vec(&Batch {
                version: 1,
                vault: self.vault.clone(),
                account: self.account.clone(),
                key_id: self.key_id.clone(),
                id: id.into(),
                body: body.to_vec(),
            })
            .map_err(|_| Error::Malformed)?,
        );
        if plain.len() > RECORD_LIMIT {
            return Err(Error::Limit);
        }
        let encryptor = age::Encryptor::with_recipients(std::iter::once(
            &self.recipient as &dyn age::Recipient,
        ))
        .map_err(|_| Error::Crypto)?;
        let mut bytes = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut bytes)
            .map_err(|_| Error::Crypto)?;
        writer.write_all(&plain).map_err(|_| Error::Crypto)?;
        writer.finish().map_err(|_| Error::Crypto)?;
        Ok(bytes)
    }
    pub fn append(&self, store: &mut Store, sealed: &[u8]) -> Result<()> {
        use sha2::{Digest, Sha256};
        if !self.enabled {
            return Err(Error::Locked);
        }
        if sealed.is_empty() || sealed.len() > MAX_SEALED {
            return Err(Error::Limit);
        }
        let snapshot = store.snapshot()?;
        if snapshot.header.as_ref().map(|h| &h.vault_id) != Some(&self.vault) {
            return Err(Error::Authentication);
        }
        let digest = hex::encode(Sha256::digest(sealed));
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let present: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM spool WHERE id=?1 AND namespace=?2)",
                params![digest, self.key_id],
                |r| r.get(0),
            )
            .map_err(failure)?;
        if !present {
            let (count, size): (u32, i64) = tx
                .query_row(
                    "SELECT count(*),coalesce(sum(length(bytes)),0) FROM spool",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(failure)?;
            if count >= 256 || size + sealed.len() as i64 > QUOTA as i64 {
                return Err(Error::Quota);
            }
            tx.execute(
                "INSERT INTO spool VALUES(?1,?2,?3)",
                params![digest, self.key_id, sealed],
            )
            .map_err(failure)?;
        }
        tx.commit().map_err(failure)
    }
    pub fn drain_one(&self, store: &mut Store, keys: &Keys) -> Result<bool> {
        self.drain_inner(store, keys, false)
    }
    fn drain_inner(
        &self,
        store: &mut Store,
        keys: &Keys,
        fail_before_commit: bool,
    ) -> Result<bool> {
        use sha2::{Digest, Sha256};
        if keys.header.vault_id != self.vault {
            return Err(Error::Authentication);
        }
        let entry: Option<(String, Vec<u8>)> = store
            .connection
            .query_row(
                "SELECT id,bytes FROM spool WHERE namespace=?2 AND length(bytes)<=?1 ORDER BY id LIMIT 1",
                params![MAX_SEALED as u32,self.key_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(failure)?;
        let Some((id, bytes)) = entry else {
            return Ok(false);
        };
        let secret_record = store.read(&self.key_id)?.ok_or(Error::Storage)?;
        let secret = keys.open(&self.account, "ingress-key", &secret_record)?;
        let identity: age::x25519::Identity = std::str::from_utf8(&secret)
            .map_err(|_| Error::Malformed)?
            .parse()
            .map_err(|_| Error::Malformed)?;
        let decryptor = age::Decryptor::new(bytes.as_slice()).map_err(|_| Error::Authentication)?;
        let reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|_| Error::Authentication)?;
        let mut plain = zeroize::Zeroizing::new(Vec::new());
        reader
            .take((RECORD_LIMIT + 1) as u64)
            .read_to_end(&mut plain)
            .map_err(|_| Error::Authentication)?;
        if plain.len() > RECORD_LIMIT {
            return Err(Error::Limit);
        }
        let mut batch: Batch = serde_json::from_slice(&plain).map_err(|_| Error::Malformed)?;
        let body = zeroize::Zeroizing::new(std::mem::take(&mut batch.body));
        if batch.version != 1
            || batch.vault != self.vault
            || batch.account != self.account
            || batch.key_id != self.key_id
            || batch.id.is_empty()
            || batch.id.len() > 128
        {
            return Err(Error::Authentication);
        }
        let receipt = keys.lookup(&self.account, &format!("ingress-receipt/{}", batch.id))?;
        let body_digest = Sha256::digest(&body);
        // Digest is encrypted too: public content guesses must not expose receipt bodies.
        let candidate = keys.seal(
            &self.account,
            &format!("untrusted-candidate/{}", batch.id),
            1,
            &body,
            &mut OsEntropy,
        )?;
        let receipt_record = keys.seal(
            &self.account,
            &format!("ingress-receipt/{}", batch.id),
            1,
            &body_digest,
            &mut OsEntropy,
        )?;
        let previous = store.read(&receipt)?;
        if let Some(previous) = &previous {
            if *keys.open(
                &self.account,
                &format!("ingress-receipt/{}", batch.id),
                previous,
            )? != body_digest.as_slice()
            {
                return Err(Error::Conflict);
            }
        }
        let revision = store
            .snapshot()?
            .revision
            .checked_add(1)
            .ok_or(Error::Limit)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        if previous.is_none() {
            for record in [&candidate, &receipt_record] {
                tx.execute(
                    "INSERT INTO records VALUES(?1,?2,?3,?4,?5)",
                    params![
                        record.key,
                        record.version,
                        record.revision,
                        record.bytes,
                        record.namespace
                    ],
                )
                .map_err(failure)?;
            }
        }
        tx.execute(
            "UPDATE metadata SET revision=?1 WHERE singleton=1",
            [revision],
        )
        .map_err(failure)?;
        tx.execute(
            "DELETE FROM spool WHERE id=?1 AND namespace=?2",
            params![id, self.key_id],
        )
        .map_err(failure)?;
        if fail_before_commit {
            return Err(Error::Quota);
        }
        tx.commit().map_err(failure)?;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::vault::Mutation;
    #[test]
    fn sealed_capture_restart_replay_and_rollback() {
        let dir = std::env::temp_dir().join(format!("nv-spool-{}", std::process::id()));
        let keys = Keys::create("spool test password", &mut OsEntropy).unwrap();
        let (grant, identity) = Grant::create(&keys, "account-a", true).unwrap();
        let mut store = Store::open(&dir).unwrap();
        store
            .commit(&Mutation {
                expected_revision: 0,
                header: Some(keys.header.clone()),
                records: vec![identity],
                create: true,
            })
            .unwrap();
        let sealed = grant.seal("batch-one", b"PRIVATE SPOOL BODY").unwrap();
        grant.append(&mut store, &sealed).unwrap();
        assert_eq!(
            grant.drain_inner(&mut store, &keys, true),
            Err(Error::Quota)
        );
        drop(store);
        let mut store = Store::open(&dir).unwrap();
        assert!(grant.drain_one(&mut store, &keys).unwrap());
        grant.append(&mut store, &sealed).unwrap();
        assert!(grant.drain_one(&mut store, &keys).unwrap());
        assert_eq!(
            store
                .page(&keys.lookup("account-a", "namespace").unwrap(), "", 16)
                .unwrap()
                .len(),
            3
        );
        let conflicting = grant.seal("batch-one", b"OTHER BODY").unwrap();
        grant.append(&mut store, &conflicting).unwrap();
        assert_eq!(grant.drain_one(&mut store, &keys), Err(Error::Conflict));
        let (disabled, _) = Grant::create(&keys, "account-a", false).unwrap();
        assert_eq!(disabled.seal("batch", b"body"), Err(Error::Locked));
        store.connection.execute("DELETE FROM spool", []).unwrap();
        let (other, _) = Grant::create(&keys, "account-b", true).unwrap();
        let wrong = other.seal("wrong-recipient", b"private").unwrap();
        grant.append(&mut store, &wrong).unwrap();
        assert_eq!(
            grant.drain_one(&mut store, &keys),
            Err(Error::Authentication)
        );
        store.connection.execute("DELETE FROM spool", []).unwrap();
        for mut damaged in [sealed.clone(), sealed[..sealed.len() - 1].to_vec()] {
            let last = damaged.len() - 1;
            damaged[last] ^= 1;
            grant.append(&mut store, &damaged).unwrap();
            assert_eq!(
                grant.drain_one(&mut store, &keys),
                Err(Error::Authentication)
            );
            store.connection.execute("DELETE FROM spool", []).unwrap();
        }
        let tx = store.connection.transaction().unwrap();
        for i in 0..256 {
            tx.execute(
                "INSERT INTO spool VALUES(?1,?2,?3)",
                params![i.to_string(), grant.key_id, vec![1u8]],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        assert_eq!(grant.append(&mut store, &sealed), Err(Error::Quota));
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
