//! Opt-in attachment bytes. A URL in a message is not a backed-up file.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_MEDIA_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaState {
    Missing,
    Stored,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaObject {
    pub hash: String,
    pub bytes: usize,
    pub host: String,
    pub state: MediaState,
}

pub fn admit(
    hash: &str,
    payload: &[u8],
    expected_hash: &str,
    host: &str,
    allowed_hosts: &[String],
) -> Result<MediaObject> {
    if payload.len() > MAX_MEDIA_BYTES {
        return Err(Error::Limit);
    }
    let actual_hash = hex::encode(Sha256::digest(payload));
    if actual_hash != expected_hash || hash != actual_hash {
        return Ok(MediaObject {
            hash: actual_hash,
            bytes: 0,
            host: host.to_string(),
            state: MediaState::Rejected,
        });
    }
    if !allowed_hosts.iter().any(|item| item == host) {
        return Err(Error::Authentication);
    }
    if payload.is_empty() {
        return Ok(MediaObject {
            hash: actual_hash,
            bytes: 0,
            host: host.to_string(),
            state: MediaState::Missing,
        });
    }
    Ok(MediaObject {
        hash: actual_hash,
        bytes: payload.len(),
        host: host.to_string(),
        state: MediaState::Stored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(payload: &[u8]) -> String {
        hex::encode(Sha256::digest(payload))
    }

    #[test]
    fn hash_mismatch_oversize_and_missing_are_distinct() {
        let hash = digest(b"img");
        let allowed = vec!["files.example".into()];
        assert_eq!(
            admit(&hash, b"nope", &digest(b"img"), "files.example", &allowed)
                .unwrap()
                .state,
            MediaState::Rejected
        );
        assert!(matches!(
            admit(
                &hash,
                &vec![0; MAX_MEDIA_BYTES + 1],
                &hash,
                "files.example",
                &allowed
            ),
            Err(Error::Limit)
        ));
        let empty = digest(b"");
        assert_eq!(
            admit(&empty, b"", &empty, "files.example", &allowed)
                .unwrap()
                .state,
            MediaState::Missing
        );
        assert_eq!(
            admit(&hash, b"img", &hash, "files.example", &allowed)
                .unwrap()
                .state,
            MediaState::Stored
        );
    }
}
