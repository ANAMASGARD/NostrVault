//! Bounded public-note recovery slice. No private-message or arbitrary-kind fan-out.
use crate::vault::{Error, Result};
pub use nostr::event::Event;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMIT: usize = 256;
pub const EVENT_BYTES: usize = 16 * 1024;
pub const SNAPSHOT_BYTES: usize = 512 * 1024;
pub const RECORD: &str = "public-note-backup-v1";
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub vault_id: String,
    pub token: String,
    pub generation: u32,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recovery {
    pub destination: String,
    pub checked_at: u64,
    pub attempted: usize,
    pub acknowledged: usize,
    pub rejected: usize,
    pub verified: usize,
    pub verification_complete: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Backup {
    pub version: u32,
    pub account: String,
    pub source: String,
    pub captured_at: u64,
    pub notes: BTreeMap<String, Event>,
    pub suppressed: BTreeSet<String>,
    pub excluded: usize,
    pub recovery: Option<Recovery>,
}
impl Backup {
    pub fn empty(account: String) -> Self {
        Self {
            version: 1,
            account,
            source: String::new(),
            captured_at: 0,
            notes: BTreeMap::new(),
            suppressed: BTreeSet::new(),
            excluded: 0,
            recovery: None,
        }
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > SNAPSHOT_BYTES {
            return Err(Error::Limit);
        }
        let data: Self = serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
        if data.version != 1 {
            return Err(Error::Unsupported);
        }
        if data.notes.len() > LIMIT || data.suppressed.len() > LIMIT * 4 {
            return Err(Error::Limit);
        }
        for (id, event) in &data.notes {
            validate(event, &data.account)?;
            if id != &event.id.to_hex() || event.kind.as_u16() != 1 {
                return Err(Error::Authentication);
            }
        }
        Ok(data)
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Malformed)?;
        if bytes.len() > SNAPSHOT_BYTES
            || self.notes.len() > LIMIT
            || self.suppressed.len() > LIMIT * 4
        {
            return Err(Error::Limit);
        }
        Ok(bytes)
    }
    pub fn prune(&mut self, now: u64) -> bool {
        let before = self.notes.len();
        self.notes
            .retain(|id, event| !self.suppressed.contains(id) && eligible(event, now));
        before != self.notes.len()
    }
    pub fn merge(&self, source: &str, events: Vec<Event>, now: u64) -> Result<Self> {
        // A saturated bounded page is not an exhaustive cursor. Preserve the old backup.
        if events.len() >= LIMIT {
            return Err(Error::Limit);
        }
        let mut next = self.clone();
        for event in &events {
            validate(event, &self.account)?;
        }
        for event in events.iter().filter(|e| e.kind.as_u16() == 5) {
            for tag in event.tags.iter() {
                let values = tag.as_slice();
                if values.first().map(String::as_str) == Some("e") {
                    if let Some(id) = values.get(1) {
                        if nostr::event::EventId::from_hex(id).is_ok() {
                            next.suppressed.insert(id.clone());
                        }
                    }
                }
            }
        }
        for event in events.into_iter().filter(|e| e.kind.as_u16() == 1) {
            let id = event.id.to_hex();
            if eligible(&event, now) && !next.suppressed.contains(&id) {
                next.notes.insert(id, event);
            } else {
                next.excluded = next.excluded.saturating_add(1);
            }
        }
        next.prune(now);
        next.source = source.into();
        next.captured_at = now;
        next.bytes()?;
        Ok(next)
    }
}
pub fn validate(event: &Event, account: &str) -> Result<()> {
    if serde_json::to_vec(event)
        .map_err(|_| Error::Malformed)?
        .len()
        > EVENT_BYTES
    {
        return Err(Error::Limit);
    }
    if event.pubkey.to_hex() != account || ![1, 5].contains(&event.kind.as_u16()) {
        return Err(Error::Authentication);
    }
    event.verify().map_err(|_| Error::Authentication)
}
pub fn eligible(event: &Event, now: u64) -> bool {
    if event.kind.as_u16() != 1 || event.created_at.as_secs() > now {
        return false;
    }
    for tag in event.tags.iter() {
        let values = tag.as_slice();
        match values.first().map(String::as_str) {
            Some("-") => return false,
            Some("expiration")
                if !values
                    .get(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_some_and(|t| t > now) =>
            {
                return false
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::prelude::{
        EventBuilder, FinalizeUnsignedEvent, Keys, Kind, SecretKey, Tag, Timestamp,
    };
    fn event(kind: u16, tags: Vec<Tag>) -> Event {
        let keys = Keys::new(
            SecretKey::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        );
        let mut unsigned = EventBuilder::new(Kind::from(kind), "PUBLIC TEST FIXTURE")
            .tags(tags)
            .custom_created_at(Timestamp::from(100))
            .finalize_unsigned(keys.public_key());
        let id = unsigned.id();
        let signature =
            keys.sign_schnorr_with_aux_rand(&secp256k1::Secp256k1::new(), id.as_bytes(), &[0; 32]);
        unsigned.add_signature(signature).unwrap()
    }
    #[test]
    fn deletion_before_target_expiration_and_protection_never_restore() {
        let note = event(1, vec![]);
        let account = note.pubkey.to_hex();
        let delete = event(5, vec![Tag::parse(["e", &note.id.to_hex()]).unwrap()]);
        let first = Backup::empty(account)
            .merge("ws://127.0.0.1:1", vec![delete], 200)
            .unwrap();
        let next = first.merge("ws://127.0.0.1:1", vec![note], 200).unwrap();
        assert!(next.notes.is_empty());
        assert!(!eligible(&event(1, vec![Tag::parse(["-"]).unwrap()]), 200));
        assert!(!eligible(
            &event(1, vec![Tag::parse(["expiration", "150"]).unwrap()]),
            200
        ));
    }
    #[test]
    fn validation_bounds_and_idempotent_capture() {
        let note = event(1, vec![]);
        let base = Backup::empty(note.pubkey.to_hex());
        let one = base
            .merge("ws://127.0.0.1:1", vec![note.clone(), note.clone()], 200)
            .unwrap();
        assert_eq!(one.notes.len(), 1);
        assert!(base.merge("", vec![note.clone(); LIMIT], 200).is_err());
        assert!(validate(&note, &"0".repeat(64)).is_err());
        let mut altered = note.clone();
        altered.content.push('x');
        assert!(validate(&altered, &base.account).is_err());
        assert_eq!(Backup::parse(&one.bytes().unwrap()).unwrap().notes.len(), 1);
    }
}
