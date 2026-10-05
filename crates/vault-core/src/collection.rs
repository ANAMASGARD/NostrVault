//! Bounded relay discovery and collection profiles (milestone 05).
use crate::vault::{Error, Result};
use nostr::event::Event;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const MAX_LOOKUP_RELAYS: usize = 3;
pub const MAX_CAPTURE_RELAYS: usize = 4;
pub const MAX_SUGGESTIONS_PER_PURPOSE: usize = 10;
pub const MAX_EVENT_BYTES: usize = 16 * 1024;

pub const SUGGESTED_LOOKUP_RELAYS: &[&str] = &["wss://relay.damus.io", "wss://nos.lol"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionProfile {
    PublicHistory,
    LegacyDirectMessages,
    GiftWraps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayPurpose {
    AuthorHistory,
    DmInbox,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayHint {
    pub url: String,
    pub purpose: RelayPurpose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionOutcome {
    InvalidAddress,
    Unavailable,
    NeedsAuthorization,
    WaitingForApproval,
    Denied,
    Empty,
    Incomplete,
    Saved,
    DiscoveryReady,
    SourceReachable,
    SourceUnavailable,
    BackupSaved,
    OfflineReady,
    RestoreVerified,
    RestorePartial,
}

impl CollectionOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAddress => "invalid_address",
            Self::Unavailable => "unavailable",
            Self::NeedsAuthorization => "needs_authorization",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::Denied => "denied",
            Self::Empty => "empty",
            Self::Incomplete => "incomplete",
            Self::Saved => "saved",
            Self::DiscoveryReady => "discovery_ready",
            Self::SourceReachable => "source_reachable",
            Self::SourceUnavailable => "source_unavailable",
            Self::BackupSaved => "backup_saved",
            Self::OfflineReady => "offline_ready",
            Self::RestoreVerified => "restore_verified",
            Self::RestorePartial => "restore_partial",
        }
    }
}

pub fn validate_lookup_relays(relays: &[String]) -> Result<()> {
    if relays.is_empty() || relays.len() > MAX_LOOKUP_RELAYS {
        return Err(Error::Limit);
    }
    let mut seen = BTreeSet::new();
    for relay in relays {
        crate::identity::relay(relay)?;
        if !seen.insert(relay.clone()) {
            return Err(Error::Malformed);
        }
    }
    Ok(())
}

pub fn validate_capture_relay(relay: &str, grants: &[String]) -> Result<()> {
    crate::identity::relay(relay)?;
    if !grants.iter().any(|g| g == relay) {
        return Err(Error::Authentication);
    }
    Ok(())
}

pub fn validate_lookup_grant(relay: &str, grants: &[String]) -> Result<()> {
    crate::identity::relay(relay)?;
    if !grants.iter().any(|g| g == relay) {
        return Err(Error::Authentication);
    }
    Ok(())
}

/// REQ filters for the profile (OR within one REQ).
pub fn profile_filters(
    profile: CollectionProfile,
    account: &str,
    until: u64,
) -> Result<Vec<Value>> {
    let limit = crate::backup::MAX_EVENTS;
    match profile {
        CollectionProfile::PublicHistory => Ok(vec![
            json!({"authors":[account],"kinds":[0,1,3,10002],"limit":limit,"until":until}),
            json!({"authors":[account],"kinds":[5],"limit":limit,"until":until}),
        ]),
        CollectionProfile::LegacyDirectMessages => Ok(vec![
            json!({"authors":[account],"kinds":[4],"limit":limit,"until":until}),
            json!({"kinds":[4],"#p":[account],"limit":limit,"until":until}),
        ]),
        CollectionProfile::GiftWraps => Ok(vec![json!({
            "kinds":[1059],
            "#p":[account],
            "limit":limit,
            "until":until
        })]),
    }
}

pub fn event_bytes(event: &Event) -> Result<usize> {
    Ok(serde_json::to_vec(event)
        .map_err(|_| Error::Malformed)?
        .len())
}

pub fn matches_profile(event: &Event, account: &str, profile: CollectionProfile) -> bool {
    let kind = event.kind.as_u16();
    let author = event.pubkey.to_hex();
    match profile {
        CollectionProfile::PublicHistory => {
            if author != account {
                return false;
            }
            matches!(kind, 0 | 1 | 3 | 10002 | 5)
        }
        CollectionProfile::LegacyDirectMessages => {
            if kind != 4 {
                return false;
            }
            if author == account {
                return true;
            }
            event.tags.iter().any(|tag| {
                tag.as_slice().first().map(String::as_str) == Some("p")
                    && tag.as_slice().get(1).map(String::as_str) == Some(account)
            })
        }
        CollectionProfile::GiftWraps => {
            if kind != 1059 {
                return false;
            }
            event.tags.iter().any(|tag| {
                tag.as_slice().first().map(String::as_str) == Some("p")
                    && tag.as_slice().get(1).map(String::as_str) == Some(account)
            })
        }
    }
}

/// Parse replaceable metadata (10002 / 10050) into typed hints.
pub fn relay_hints_from_metadata(event: &Event, account: &str) -> Result<Vec<RelayHint>> {
    if event.pubkey.to_hex() != account {
        return Err(Error::Authentication);
    }
    event.verify().map_err(|_| Error::Authentication)?;
    let kind = event.kind.as_u16();
    let mut out = Vec::new();
    match kind {
        10002 => {
            for tag in event.tags.iter() {
                let values = tag.as_slice();
                if values.first().map(String::as_str) != Some("r") {
                    continue;
                }
                let Some(url) = values.get(1) else {
                    continue;
                };
                let marker = values.get(2).map(String::as_str);
                if marker == Some("read") {
                    continue;
                }
                crate::identity::relay(url)?;
                out.push(RelayHint {
                    url: url.clone(),
                    purpose: RelayPurpose::AuthorHistory,
                });
                if out.len() >= MAX_SUGGESTIONS_PER_PURPOSE {
                    break;
                }
            }
        }
        10050 => {
            for tag in event.tags.iter() {
                let values = tag.as_slice();
                if values.first().map(String::as_str) != Some("relay") {
                    continue;
                }
                let Some(url) = values.get(1) else {
                    continue;
                };
                crate::identity::relay(url)?;
                out.push(RelayHint {
                    url: url.clone(),
                    purpose: RelayPurpose::DmInbox,
                });
                if out.len() >= MAX_SUGGESTIONS_PER_PURPOSE {
                    break;
                }
            }
        }
        _ => return Err(Error::Unsupported),
    }
    Ok(out)
}

/// NIP-01 replaceable winner: highest created_at, then lowest id.
pub fn replaceable_winner<'a>(events: impl IntoIterator<Item = &'a Event>) -> Option<&'a Event> {
    events.into_iter().max_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| b.id.cmp(&a.id))
    })
}

pub fn merge_hint_lists(
    history: &mut BTreeSet<String>,
    inbox: &mut BTreeSet<String>,
    hints: &[RelayHint],
) {
    for hint in hints {
        match hint.purpose {
            RelayPurpose::AuthorHistory => {
                if history.len() >= MAX_SUGGESTIONS_PER_PURPOSE {
                    continue;
                }
                history.insert(hint.url.clone());
            }
            RelayPurpose::DmInbox => {
                if inbox.len() >= MAX_SUGGESTIONS_PER_PURPOSE {
                    continue;
                }
                inbox.insert(hint.url.clone());
            }
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverySuggestions {
    pub history: Vec<String>,
    pub inbox: Vec<String>,
}

pub fn suggestions_from_maps(
    history: &BTreeSet<String>,
    inbox: &BTreeSet<String>,
) -> DiscoverySuggestions {
    DiscoverySuggestions {
        history: history
            .iter()
            .take(MAX_SUGGESTIONS_PER_PURPOSE)
            .cloned()
            .collect(),
        inbox: inbox
            .iter()
            .take(MAX_SUGGESTIONS_PER_PURPOSE)
            .cloned()
            .collect(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialJobState {
    Pending,
    Running,
    Saved,
    Incomplete,
    NeedsAttention,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitialCollectionJob {
    pub version: u32,
    pub profile: CollectionProfile,
    pub relay: String,
    pub consent_revision: u32,
    pub state: InitialJobState,
    pub last_attempt: u64,
}

impl InitialCollectionJob {
    pub const RECORD: &'static str = "initial-collection-job-v1";

    pub fn new(profile: CollectionProfile, relay: String, consent_revision: u32, now: u64) -> Self {
        Self {
            version: 1,
            profile,
            relay,
            consent_revision,
            state: InitialJobState::Pending,
            last_attempt: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::prelude::{
        EventBuilder, FinalizeUnsignedEvent, Keys, Kind, SecretKey, Tag, Timestamp,
    };

    fn signed(kind: u16, tags: Vec<Tag>, created: u64) -> Event {
        let keys = Keys::new(
            SecretKey::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        );
        let mut unsigned = EventBuilder::new(Kind::from(kind), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(created))
            .finalize_unsigned(keys.public_key());
        let id = unsigned.id();
        let signature =
            keys.sign_schnorr_with_aux_rand(&secp256k1::Secp256k1::new(), id.as_bytes(), &[0; 32]);
        unsigned.add_signature(signature).unwrap()
    }

    #[test]
    fn nip65_write_and_unmarked_are_history_read_only_is_not() {
        let account = signed(10002, vec![], 100).pubkey.to_hex();
        let nip65 = signed(
            10002,
            vec![
                Tag::parse(["r", "wss://history.example"]).unwrap(),
                Tag::parse(["r", "wss://write.example", "write"]).unwrap(),
                Tag::parse(["r", "wss://read.example", "read"]).unwrap(),
            ],
            100,
        );
        let hints = relay_hints_from_metadata(&nip65, &account).unwrap();
        assert_eq!(hints.len(), 2);
        assert!(hints
            .iter()
            .all(|h| h.purpose == RelayPurpose::AuthorHistory));
    }

    #[test]
    fn kind_10050_uses_relay_tag_not_r() {
        let account = signed(10050, vec![], 100).pubkey.to_hex();
        let with_r = signed(
            10050,
            vec![Tag::parse(["r", "wss://wrong.example"]).unwrap()],
            100,
        );
        assert!(relay_hints_from_metadata(&with_r, &account)
            .unwrap()
            .is_empty());
        let dm = signed(
            10050,
            vec![Tag::parse(["relay", "wss://inbox.example"]).unwrap()],
            100,
        );
        let hints = relay_hints_from_metadata(&dm, &account).unwrap();
        assert_eq!(hints[0].purpose, RelayPurpose::DmInbox);
    }

    #[test]
    fn replaceable_winner_picks_newest_then_lowest_id() {
        let old = signed(10002, vec![], 100);
        let new = signed(10002, vec![], 200);
        let winner = replaceable_winner([&old, &new]).unwrap();
        assert_eq!(winner.created_at.as_secs(), 200);
    }

    #[test]
    fn gift_wrap_and_kind4_matching() {
        let account = signed(1, vec![], 100).pubkey.to_hex();
        let wrap_keys = Keys::new(
            SecretKey::from_hex("0000000000000000000000000000000000000000000000000000000000000002")
                .unwrap(),
        );
        let mut unsigned = EventBuilder::new(Kind::from(1059), "cipher")
            .tags(vec![Tag::parse(["p", &account]).unwrap()])
            .custom_created_at(Timestamp::from(100))
            .finalize_unsigned(wrap_keys.public_key());
        let id = unsigned.id();
        let signature = wrap_keys.sign_schnorr_with_aux_rand(
            &secp256k1::Secp256k1::new(),
            id.as_bytes(),
            &[0; 32],
        );
        let wrap = unsigned.add_signature(signature).unwrap();
        assert!(matches_profile(
            &wrap,
            &account,
            CollectionProfile::GiftWraps
        ));
        let incoming = signed(4, vec![Tag::parse(["p", &account]).unwrap()], 100);
        assert!(matches_profile(
            &incoming,
            &account,
            CollectionProfile::LegacyDirectMessages
        ));
    }
}
