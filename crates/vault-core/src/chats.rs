//! Conversation projections from already-decoded rumor candidates.
//! Archive decryption is not local authorship proof. Display suppresses
//! deleted and expired bodies without discarding the suppression record.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecryptState {
    Pending,
    Failed,
    Verified,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RumorCandidate {
    pub rumor_id: String,
    pub outer_id: String,
    pub sender: String,
    pub claimed_sender: String,
    pub created_at: u64,
    pub participants: Vec<String>,
    pub body: String,
    pub deleted: bool,
    pub expired: bool,
    pub state: DecryptState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalMessage {
    pub rumor_id: String,
    pub outer_ids: Vec<String>,
    pub sender: String,
    pub created_at: u64,
    pub participants: Vec<String>,
    pub body: String,
    pub state: DecryptState,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub participants: Vec<String>,
    pub label: String,
    pub messages: Vec<LogicalMessage>,
}

fn bounded(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max {
        return Err(Error::Limit);
    }
    Ok(())
}

/// Collapse duplicate outer wraps of one rumor. Sender/rumor mismatch fails closed.
pub fn project_rumors(items: &[RumorCandidate]) -> Result<Vec<LogicalMessage>> {
    if items.len() > 256 {
        return Err(Error::Limit);
    }
    let mut grouped: BTreeMap<String, LogicalMessage> = BTreeMap::new();
    for item in items {
        bounded(&item.rumor_id, 128)?;
        bounded(&item.outer_id, 128)?;
        bounded(&item.sender, 128)?;
        if item.sender != item.claimed_sender {
            return Err(Error::Malformed);
        }
        if item.participants.len() > 16 {
            return Err(Error::Limit);
        }
        let mut participants = item.participants.clone();
        participants.sort();
        participants.dedup();
        let visible = item.state == DecryptState::Verified && !item.deleted && !item.expired;
        let body = if visible {
            if item.body.len() > 16_384 {
                return Err(Error::Limit);
            }
            item.body.clone()
        } else {
            String::new()
        };
        if let Some(existing) = grouped.get_mut(&item.rumor_id) {
            if existing.sender != item.sender
                || existing.created_at != item.created_at
                || existing.participants != participants
            {
                return Err(Error::Conflict);
            }
            if !existing.outer_ids.contains(&item.outer_id) {
                existing.outer_ids.push(item.outer_id.clone());
                existing.outer_ids.sort();
            }
            if item.state == DecryptState::Verified {
                existing.state = DecryptState::Verified;
                existing.body = body;
                existing.visible = visible;
            }
        } else {
            let mut outer_ids = vec![item.outer_id.clone()];
            outer_ids.sort();
            grouped.insert(
                item.rumor_id.clone(),
                LogicalMessage {
                    rumor_id: item.rumor_id.clone(),
                    outer_ids,
                    sender: item.sender.clone(),
                    created_at: item.created_at,
                    participants,
                    body,
                    state: item.state,
                    visible,
                },
            );
        }
    }
    Ok(grouped.into_values().collect())
}

pub fn conversations(
    messages: &[LogicalMessage],
    aliases: &BTreeMap<String, String>,
) -> Vec<Conversation> {
    let mut groups: BTreeMap<String, Conversation> = BTreeMap::new();
    for message in messages {
        let id = message.participants.join(":");
        let label = message
            .participants
            .iter()
            .map(|key| {
                aliases
                    .get(key)
                    .cloned()
                    .filter(|alias| !alias.is_empty())
                    .unwrap_or_else(|| abbreviate(key))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let entry = groups.entry(id.clone()).or_insert_with(|| Conversation {
            id,
            participants: message.participants.clone(),
            label,
            messages: Vec::new(),
        });
        entry.messages.push(message.clone());
    }
    for conversation in groups.values_mut() {
        conversation.messages.sort_by_key(|m| m.created_at);
    }
    groups.into_values().collect()
}

fn abbreviate(key: &str) -> String {
    let end = key.chars().take(8).collect::<String>();
    format!("{end}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rumor(id: &str, outer: &str, state: DecryptState) -> RumorCandidate {
        RumorCandidate {
            rumor_id: id.into(),
            outer_id: outer.into(),
            sender: "a".into(),
            claimed_sender: "a".into(),
            created_at: 100,
            participants: vec!["a".into(), "b".into()],
            body: "hello".into(),
            deleted: false,
            expired: false,
            state,
        }
    }

    #[test]
    fn two_wraps_one_logical_message() {
        let messages = project_rumors(&[
            rumor("r1", "w2", DecryptState::Verified),
            rumor("r1", "w1", DecryptState::Pending),
        ])
        .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].outer_ids,
            vec!["w1".to_string(), "w2".to_string()]
        );
        assert_eq!(messages[0].state, DecryptState::Verified);
        assert!(messages[0].visible);
    }

    #[test]
    fn sender_mismatch_rejected() {
        let mut bad = rumor("r1", "w1", DecryptState::Verified);
        bad.claimed_sender = "other".into();
        assert!(matches!(project_rumors(&[bad]), Err(Error::Malformed)));
    }

    #[test]
    fn pending_failed_and_suppressed_are_not_visible_text() {
        let mut failed = rumor("r2", "w", DecryptState::Failed);
        failed.body = "secret".into();
        let mut deleted = rumor("r3", "w", DecryptState::Verified);
        deleted.deleted = true;
        let pending = rumor("r4", "w", DecryptState::Pending);
        let messages = project_rumors(&[failed, deleted, pending]).unwrap();
        assert!(messages.iter().all(|m| m.body.is_empty() && !m.visible));
        assert!(messages.iter().any(|m| m.state == DecryptState::Failed));
        assert!(messages.iter().any(|m| m.state == DecryptState::Pending));
    }

    #[test]
    fn alias_does_not_merge_distinct_participant_sets() {
        let mut first = rumor("r1", "w1", DecryptState::Verified);
        first.participants = vec!["a".into(), "b".into()];
        let mut second = rumor("r2", "w2", DecryptState::Verified);
        second.participants = vec!["a".into(), "c".into()];
        second.created_at = 200;
        let messages = project_rumors(&[first, second]).unwrap();
        let mut aliases = BTreeMap::new();
        aliases.insert("b".into(), "Sam".into());
        aliases.insert("c".into(), "Sam".into());
        let threads = conversations(&messages, &aliases);
        assert_eq!(threads.len(), 2);
        assert!(threads.iter().all(|t| t.label.contains("Sam")));
    }
}
