//! Restore outcomes. An acknowledgement is not independent read-back.
//! Private events are not eligible for a public destination.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Acknowledged,
    Verified,
    Rejected,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventOutcome {
    pub id: String,
    pub outcome: Outcome,
}

pub fn classify(acknowledged: bool, read_back: bool, rejected: bool) -> Outcome {
    if rejected {
        return Outcome::Rejected;
    }
    if read_back {
        return Outcome::Verified;
    }
    if acknowledged {
        return Outcome::Acknowledged;
    }
    Outcome::Unknown
}

pub fn allow_private_restore(inbox_granted: bool) -> Result<()> {
    if inbox_granted {
        Ok(())
    } else {
        Err(Error::Authentication)
    }
}

pub fn summary(verified: usize, eligible: usize) -> String {
    format!("{verified} of {eligible} eligible events were returned and validated from the selected relay at the recorded check time.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lost_ack_is_not_verified_and_rejection_stays_partial() {
        assert_eq!(classify(true, false, false), Outcome::Acknowledged);
        assert_eq!(classify(false, true, false), Outcome::Verified);
        assert_eq!(classify(true, false, true), Outcome::Rejected);
        let rows = [classify(true, true, false), classify(true, false, true)];
        assert!(rows.contains(&Outcome::Verified));
        assert!(rows.contains(&Outcome::Rejected));
        assert_ne!(
            rows.iter().filter(|o| **o == Outcome::Verified).count(),
            rows.len()
        );
    }

    #[test]
    fn private_restore_requires_inbox_grant() {
        assert!(allow_private_restore(false).is_err());
        assert!(allow_private_restore(true).is_ok());
    }
}
