//! Bounded raw-event backup snapshot (milestone 05).
use crate::collection::{matches_profile, CollectionProfile, MAX_EVENT_BYTES};
use crate::vault::{Error, Result};
pub use nostr::event::Event;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EVENTS: usize = 256;
pub const MAX_RAW_BYTES: usize = 768 * 1024;
pub const SEAL_BYTES: usize = 1024 * 1024;
pub const LEGACY_RECORD: &str = "public-note-backup-v1";
pub const RECORD: &str = "raw-event-backup-v2";

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
pub struct BackupSnapshot {
    pub version: u32,
    pub account: String,
    pub source: String,
    pub captured_at: u64,
    pub profile: CollectionProfile,
    pub events: BTreeMap<String, Event>,
    pub suppressed: BTreeSet<String>,
    pub rejected: usize,
    pub excluded: usize,
    pub recovery: Option<Recovery>,
    #[serde(default)]
    pub provenance: BTreeMap<String, BTreeSet<String>>,
}

pub struct MergeInput<'a> {
    pub source: &'a str,
    pub profile: CollectionProfile,
    pub events: Vec<Event>,
    pub rejected: usize,
    pub now: u64,
    pub incomplete: bool,
}

pub struct MergeResult {
    pub snapshot: BackupSnapshot,
    pub stopped: bool,
}

impl BackupSnapshot {
    pub fn empty(account: String) -> Self {
        Self {
            version: 2,
            account,
            source: String::new(),
            captured_at: 0,
            profile: CollectionProfile::PublicHistory,
            events: BTreeMap::new(),
            suppressed: BTreeSet::new(),
            rejected: 0,
            excluded: 0,
            recovery: None,
            provenance: BTreeMap::new(),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > SEAL_BYTES {
            return Err(Error::Limit);
        }
        let data: Self = serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
        if data.version != 2 {
            return Err(Error::Unsupported);
        }
        data.validate_integrity()?;
        Ok(data)
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        self.validate_integrity()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Malformed)?;
        if bytes.len() > SEAL_BYTES {
            return Err(Error::Limit);
        }
        Ok(bytes)
    }

    fn validate_integrity(&self) -> Result<()> {
        if self.events.len() > MAX_EVENTS || self.suppressed.len() > MAX_EVENTS * 4 {
            return Err(Error::Limit);
        }
        let mut raw = 0usize;
        for (id, event) in &self.events {
            if id != &event.id.to_hex() {
                return Err(Error::Authentication);
            }
            raw = raw.saturating_add(event_bytes(event)?);
            if raw > MAX_RAW_BYTES {
                return Err(Error::Limit);
            }
        }
        Ok(())
    }

    pub fn raw_bytes(&self) -> Result<usize> {
        self.events.values().try_fold(0usize, |acc, e| {
            event_bytes(e).map(|b| acc.saturating_add(b))
        })
    }

    pub fn prune_public(&mut self, now: u64) -> bool {
        let before = self.events.len();
        self.events.retain(|id, event| {
            if self.suppressed.contains(id) {
                return false;
            }
            if event.kind.as_u16() == 1 {
                return public_note_eligible(event, now);
            }
            true
        });
        before != self.events.len()
    }

    pub fn merge_incoming(&self, input: MergeInput<'_>) -> Result<MergeResult> {
        let mut next = self.clone();
        next.profile = input.profile;
        let mut stopped = false;
        let batch = input.events;
        for event in &batch {
            if next.events.len() >= MAX_EVENTS {
                stopped = true;
                break;
            }
            if !try_accept_event(&mut next, event, input.source, input.profile, input.now)? {
                next.rejected = next.rejected.saturating_add(1);
            }
        }
        next.rejected = next.rejected.saturating_add(input.rejected);
        apply_deletions(&mut next, &batch);
        next.source = input.source.into();
        next.captured_at = input.now;
        next.prune_public(input.now);
        if next.raw_bytes()? > MAX_RAW_BYTES {
            stopped = true;
        }
        next.bytes()?;
        Ok(MergeResult {
            snapshot: next,
            stopped: stopped || input.incomplete,
        })
    }
}

fn event_bytes(event: &Event) -> Result<usize> {
    Ok(serde_json::to_vec(event)
        .map_err(|_| Error::Malformed)?
        .len())
}

fn try_accept_event(
    snap: &mut BackupSnapshot,
    event: &Event,
    source: &str,
    profile: CollectionProfile,
    now: u64,
) -> Result<bool> {
    if event_bytes(event)? > MAX_EVENT_BYTES {
        snap.excluded = snap.excluded.saturating_add(1);
        return Ok(false);
    }
    if event.verify().is_err() {
        snap.excluded = snap.excluded.saturating_add(1);
        return Ok(false);
    }
    if event.created_at.as_secs() > now {
        snap.excluded = snap.excluded.saturating_add(1);
        return Ok(false);
    }
    if !matches_profile(event, &snap.account, profile) {
        snap.excluded = snap.excluded.saturating_add(1);
        return Ok(false);
    }
    let id = event.id.to_hex();
    if snap.events.contains_key(&id) {
        snap.provenance
            .entry(id)
            .or_default()
            .insert(source.to_string());
        return Ok(true);
    }
    if snap.events.len() >= MAX_EVENTS {
        return Ok(false);
    }
    let added_bytes = snap.raw_bytes()?.saturating_add(event_bytes(event)?);
    if added_bytes > MAX_RAW_BYTES {
        return Ok(false);
    }
    snap.events.insert(id.clone(), event.clone());
    snap.provenance
        .entry(id)
        .or_default()
        .insert(source.to_string());
    Ok(true)
}

fn apply_deletions(snap: &mut BackupSnapshot, events: &[Event]) {
    for event in events.iter().filter(|e| e.kind.as_u16() == 5) {
        if event.pubkey.to_hex() != snap.account {
            continue;
        }
        for tag in event.tags.iter() {
            let values = tag.as_slice();
            if values.first().map(String::as_str) == Some("e") {
                if let Some(id) = values.get(1) {
                    if nostr::event::EventId::from_hex(id).is_ok() {
                        snap.suppressed.insert(id.clone());
                    }
                }
            }
        }
    }
}

pub fn public_note_eligible(event: &Event, now: u64) -> bool {
    if event.kind.as_u16() != 1 || event.created_at.as_secs() > now {
        return false;
    }
    for tag in event.tags.iter() {
        let values = tag.as_slice();
        match values.first().map(String::as_str) {
            Some("-") => return false,
            Some("expiration")
                if values
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

pub fn restore_eligible_m05(event: &Event, suppressed: &BTreeSet<String>, now: u64) -> bool {
    let id = event.id.to_hex();
    if suppressed.contains(&id) {
        return false;
    }
    public_note_eligible(event, now)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyBackup {
    version: u32,
    account: String,
    source: String,
    captured_at: u64,
    notes: BTreeMap<String, Event>,
    suppressed: BTreeSet<String>,
    excluded: usize,
    recovery: Option<Recovery>,
    #[serde(default)]
    provenance: BTreeMap<String, BTreeSet<String>>,
}

pub fn migrate_legacy_v1(bytes: &[u8]) -> Result<BackupSnapshot> {
    if bytes.len() > SEAL_BYTES {
        return Err(Error::Limit);
    }
    let legacy: LegacyBackup = serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
    if legacy.version != 1 {
        return Err(Error::Unsupported);
    }
    let snap = BackupSnapshot {
        version: 2,
        account: legacy.account,
        source: legacy.source,
        captured_at: legacy.captured_at,
        profile: CollectionProfile::PublicHistory,
        events: legacy.notes,
        suppressed: legacy.suppressed,
        excluded: legacy.excluded,
        recovery: legacy.recovery,
        provenance: legacy.provenance,
        rejected: 0,
    };
    snap.validate_integrity()?;
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::CollectionProfile;
    use nostr::prelude::{
        EventBuilder, FinalizeUnsignedEvent, Keys, Kind, SecretKey, Tag, Timestamp,
    };

    fn event(kind: u16, tags: Vec<Tag>) -> Event {
        event_with_time(kind, tags, 100)
    }
    fn event_with_time(kind: u16, tags: Vec<Tag>, created: u64) -> Event {
        signed_note_kind(kind, created, "PUBLIC TEST FIXTURE", tags)
    }
    fn signed_note(created: u64, content: &str) -> Event {
        signed_note_kind(1, created, content, Vec::<Tag>::new())
    }
    fn signed_note_kind(kind: u16, created: u64, content: &str, tags: Vec<Tag>) -> Event {
        let keys = Keys::new(
            SecretKey::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        );
        let mut unsigned = EventBuilder::new(Kind::from(kind), content)
            .tags(tags)
            .custom_created_at(Timestamp::from(created))
            .finalize_unsigned(keys.public_key());
        let id = unsigned.id();
        let signature =
            keys.sign_schnorr_with_aux_rand(&secp256k1::Secp256k1::new(), id.as_bytes(), &[0; 32]);
        unsigned.add_signature(signature).unwrap()
    }

    #[test]
    fn per_event_rejection_does_not_drop_valid_batch() {
        let account = event(1, vec![]).pubkey.to_hex();
        let base = BackupSnapshot::empty(account);
        let mut batch: Vec<Event> = (0..100)
            .map(|i| signed_note(100 + i, &format!("note-{i}")))
            .collect();
        batch.push(signed_note(50, "tampered"));
        let mut bad = batch.last().unwrap().clone();
        bad.content.push('x');
        *batch.last_mut().unwrap() = bad;
        batch.extend((0..20).map(|i| signed_note(200 + i, &format!("tail-{i}"))));
        let result = base
            .merge_incoming(MergeInput {
                source: "ws://127.0.0.1:1",
                profile: CollectionProfile::PublicHistory,
                events: batch,
                rejected: 0,
                now: 200,
                incomplete: false,
            })
            .unwrap();
        assert!(result.snapshot.events.len() >= 100);
        assert!(result.snapshot.excluded >= 1);
    }

    #[test]
    fn duplicate_provenance_and_restore_eligibility() {
        let note = event(1, vec![]);
        let account = note.pubkey.to_hex();
        let base = BackupSnapshot::empty(account);
        let one = base
            .merge_incoming(MergeInput {
                source: "wss://a.example",
                profile: CollectionProfile::PublicHistory,
                events: vec![note.clone()],
                rejected: 0,
                now: 200,
                incomplete: false,
            })
            .unwrap()
            .snapshot;
        let two = one
            .merge_incoming(MergeInput {
                source: "wss://b.example",
                profile: CollectionProfile::PublicHistory,
                events: vec![note.clone()],
                rejected: 0,
                now: 200,
                incomplete: false,
            })
            .unwrap()
            .snapshot;
        assert_eq!(two.provenance.values().next().unwrap().len(), 2);
        assert!(restore_eligible_m05(&note, &BTreeSet::new(), 200));
        let k4 = event(4, vec![Tag::parse(["p", &two.account]).unwrap()]);
        assert!(!restore_eligible_m05(&k4, &BTreeSet::new(), 200));
    }

    #[test]
    fn legacy_v1_migrates_to_v2() {
        let note = event(1, vec![]);
        let account = note.pubkey.to_hex();
        let legacy = LegacyBackup {
            version: 1,
            account,
            source: "ws://127.0.0.1:1".into(),
            captured_at: 100,
            notes: BTreeMap::from([(note.id.to_hex(), note)]),
            suppressed: BTreeSet::new(),
            excluded: 0,
            recovery: None,
            provenance: BTreeMap::new(),
        };
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let v2 = migrate_legacy_v1(&bytes).unwrap();
        assert_eq!(v2.version, 2);
        assert_eq!(v2.events.len(), 1);
    }
}
