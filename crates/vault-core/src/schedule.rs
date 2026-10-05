//! Approximate catch-up schedule shared by Android WorkManager-shaped jobs.
//! Force-stop still stops work. This does not decrypt in the background.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MIN_INTERVAL_MINUTES: u32 = 15;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUpJob {
    pub id: String,
    pub interval_minutes: u32,
    pub requires_network: bool,
    pub decrypts: bool,
}

pub fn plan(account: &str, interval_minutes: u32) -> Result<CatchUpJob> {
    if account.is_empty() || account.len() > 128 || interval_minutes < MIN_INTERVAL_MINUTES {
        return Err(Error::Limit);
    }
    Ok(CatchUpJob {
        id: format!("catch-up:{account}"),
        interval_minutes,
        requires_network: true,
        decrypts: false,
    })
}

pub fn ensure_unique(seen: &mut BTreeSet<String>, job: &CatchUpJob) -> Result<()> {
    if !seen.insert(job.id.clone()) {
        return Ok(());
    }
    if seen.len() > 8 {
        seen.remove(&job.id);
        return Err(Error::Limit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_keep_one_id_and_do_not_decrypt() {
        let job = plan("acct", 15).unwrap();
        assert!(!job.decrypts);
        assert!(plan("acct", 14).is_err());
        let mut seen = BTreeSet::new();
        ensure_unique(&mut seen, &job).unwrap();
        ensure_unique(&mut seen, &job).unwrap();
        assert_eq!(seen.len(), 1);
    }
}
