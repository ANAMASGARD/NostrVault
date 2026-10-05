//! Durable collection checkpoints. Accepted events must be stored before the
//! cursor moves. Job ids are idempotent per account, relay, and filter.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_OVERLAP_SECS: u64 = 7 * 24 * 60 * 60;
pub const MAX_ATTEMPTS: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Running,
    Paused,
    NeedsAuth,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncJob {
    pub id: String,
    pub account: String,
    pub relay: String,
    pub filter_key: String,
    pub until: u64,
    pub overlap_secs: u64,
    pub stored: bool,
    pub attempt: u32,
    pub state: SyncState,
}

pub fn job_id(account: &str, relay: &str, filter_key: &str) -> Result<String> {
    for part in [account, relay, filter_key] {
        if part.is_empty() || part.len() > 256 {
            return Err(Error::Limit);
        }
    }
    Ok(format!("{account}|{relay}|{filter_key}"))
}

pub fn open_job(account: &str, relay: &str, filter_key: &str, now: u64) -> Result<SyncJob> {
    Ok(SyncJob {
        id: job_id(account, relay, filter_key)?,
        account: account.to_string(),
        relay: relay.to_string(),
        filter_key: filter_key.to_string(),
        until: now.saturating_sub(DEFAULT_OVERLAP_SECS),
        overlap_secs: DEFAULT_OVERLAP_SECS,
        stored: false,
        attempt: 0,
        state: SyncState::Running,
    })
}

/// Insert only when absent so a remount does not fork a second job.
pub fn ensure_job(jobs: &mut BTreeMap<String, SyncJob>, job: SyncJob) -> Result<()> {
    if jobs.len() >= 32 && !jobs.contains_key(&job.id) {
        return Err(Error::Limit);
    }
    jobs.entry(job.id.clone()).or_insert(job);
    Ok(())
}

pub fn note_stored(job: &mut SyncJob) {
    job.stored = true;
}

/// Move the checkpoint only after storage succeeded.
pub fn advance(job: &mut SyncJob, next_until: u64) -> Result<()> {
    if job.state == SyncState::Cancelled {
        return Err(Error::Cancelled);
    }
    if !job.stored {
        return Err(Error::Conflict);
    }
    if next_until < job.until {
        return Err(Error::Malformed);
    }
    job.until = next_until.saturating_sub(job.overlap_secs.min(next_until));
    job.stored = false;
    job.attempt = 0;
    job.state = SyncState::Running;
    Ok(())
}

pub fn backoff_ms(attempt: u32) -> u64 {
    let shift = attempt.min(8);
    250u64.saturating_mul(1u64 << shift).min(30_000)
}

pub fn fail_attempt(job: &mut SyncJob, needs_auth: bool) -> Result<()> {
    if job.state == SyncState::Cancelled {
        return Err(Error::Cancelled);
    }
    job.attempt = job.attempt.saturating_add(1);
    job.state = if needs_auth {
        SyncState::NeedsAuth
    } else if job.attempt >= MAX_ATTEMPTS {
        SyncState::Paused
    } else {
        SyncState::Running
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_does_not_move_before_store() {
        let mut job = open_job("acct", "ws://a", "kind1", 1_000_000).unwrap();
        assert!(matches!(advance(&mut job, 1_000_100), Err(Error::Conflict)));
        note_stored(&mut job);
        advance(&mut job, 1_000_100).unwrap();
        assert!(job.until < 1_000_100);
        assert!(!job.stored);
    }

    #[test]
    fn repeated_ensure_keeps_one_job() {
        let mut jobs = BTreeMap::new();
        let job = open_job("acct", "ws://a", "kind1", 10).unwrap();
        let id = job.id.clone();
        ensure_job(&mut jobs, job.clone()).unwrap();
        let mut again = job;
        again.attempt = 4;
        ensure_job(&mut jobs, again).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[&id].attempt, 0);
    }

    #[test]
    fn backoff_is_bounded() {
        assert!(backoff_ms(0) < backoff_ms(3));
        assert_eq!(backoff_ms(20), 30_000);
    }
}
