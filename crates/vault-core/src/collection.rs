//! Bounded relay discovery and collection profiles (milestone 05).
use crate::vault::{Error, Result};
use nostr::event::Event;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_LOOKUP_RELAYS: usize = 3;
pub const MAX_HISTORY_SUGGESTIONS: usize = 10;

pub const SUGGESTED_LOOKUP_RELAYS: &[&str] = &["wss://relay.damus.io", "wss://nos.lol"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionProfile {
    PublicHistory,
    LegacyDirectMessages,
    GiftWraps,
}

impl CollectionProfile {
    pub fn implemented(self) -> bool {
        matches!(self, Self::PublicHistory)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayHint {
    pub url: String,
    pub from_kind: u16,
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

/// Parse kind 10002 (NIP-65) or 10050 (DM inbox) relay hints from `r` tags only.
pub fn relay_hints_from_metadata(event: &Event, account: &str) -> Result<Vec<RelayHint>> {
    if event.pubkey.to_hex() != account {
        return Err(Error::Authentication);
    }
    let kind = event.kind.as_u16();
    if kind != 10002 && kind != 10050 {
        return Err(Error::Unsupported);
    }
    event.verify().map_err(|_| Error::Authentication)?;
    let mut out = Vec::new();
    for tag in event.tags.iter() {
        let values = tag.as_slice();
        if values.first().map(String::as_str) != Some("r") {
            continue;
        }
        let Some(url) = values.get(1) else {
            continue;
        };
        crate::identity::relay(url)?;
        out.push(RelayHint {
            url: url.clone(),
            from_kind: kind,
        });
        if out.len() >= MAX_HISTORY_SUGGESTIONS {
            break;
        }
    }
    Ok(out)
}

pub fn merge_suggestions(existing: &mut BTreeSet<String>, hints: &[RelayHint]) {
    for hint in hints {
        if existing.len() >= MAX_HISTORY_SUGGESTIONS {
            break;
        }
        existing.insert(hint.url.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::prelude::{
        EventBuilder, FinalizeUnsignedEvent, Keys, Kind, SecretKey, Tag, Timestamp,
    };

    fn signed(kind: u16, tags: Vec<Tag>) -> Event {
        let keys = Keys::new(
            SecretKey::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        );
        let mut unsigned = EventBuilder::new(Kind::from(kind), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(100))
            .finalize_unsigned(keys.public_key());
        let id = unsigned.id();
        let signature =
            keys.sign_schnorr_with_aux_rand(&secp256k1::Secp256k1::new(), id.as_bytes(), &[0; 32]);
        unsigned.add_signature(signature).unwrap()
    }

    #[test]
    fn distinguishes_nip65_and_dm_inbox_kinds() {
        let account = signed(10002, vec![]).pubkey.to_hex();
        let nip65 = signed(
            10002,
            vec![
                Tag::parse(["r", "wss://history.example"]).unwrap(),
                Tag::parse(["r", "wss://read.example"]).unwrap(),
            ],
        );
        let dm = signed(10050, vec![Tag::parse(["r", "wss://dm.example"]).unwrap()]);
        let hints65 = relay_hints_from_metadata(&nip65, &account).unwrap();
        assert_eq!(hints65.len(), 2);
        assert!(hints65.iter().all(|h| h.from_kind == 10002));
        let hints50 = relay_hints_from_metadata(&dm, &account).unwrap();
        assert_eq!(hints50[0].from_kind, 10050);
        assert!(relay_hints_from_metadata(&nip65, &"0".repeat(64)).is_err());
    }

    #[test]
    fn lookup_relay_list_is_bounded_and_deduplicated() {
        let relays: Vec<String> = SUGGESTED_LOOKUP_RELAYS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        validate_lookup_relays(&relays).unwrap();
        assert!(validate_lookup_relays(&[]).is_err());
        assert!(validate_lookup_relays(&vec!["ws://127.0.0.1:1".into(); 4]).is_err());
        let mut merged = BTreeSet::new();
        merge_suggestions(
            &mut merged,
            &[
                RelayHint {
                    url: "wss://a.example".into(),
                    from_kind: 10002,
                },
                RelayHint {
                    url: "wss://a.example".into(),
                    from_kind: 10050,
                },
            ],
        );
        assert_eq!(merged.len(), 1);
    }
}
