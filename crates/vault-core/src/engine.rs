//! Window lifecycle for the Linux engine. One database. Readable decrypt pauses
//! when the window closes unless the user opted into encrypted capture only.
use crate::vault::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPolicy {
    pub keep_running: bool,
    pub capture_while_locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowClose {
    PauseReadable,
    ContinueEncryptedCapture,
    Stop,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            keep_running: false,
            capture_while_locked: false,
        }
    }
}

pub fn on_window_close(policy: &SessionPolicy) -> WindowClose {
    if !policy.keep_running {
        return WindowClose::Stop;
    }
    if policy.capture_while_locked {
        WindowClose::ContinueEncryptedCapture
    } else {
        WindowClose::PauseReadable
    }
}

pub fn reopen(saved: &SessionPolicy) -> Result<SessionPolicy> {
    if saved.capture_while_locked && !saved.keep_running {
        return Err(Error::Malformed);
    }
    Ok(saved.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_close_and_reopen_keep_the_same_policy() {
        let mut policy = SessionPolicy {
            keep_running: true,
            capture_while_locked: true,
        };
        assert_eq!(
            on_window_close(&policy),
            WindowClose::ContinueEncryptedCapture
        );
        policy.capture_while_locked = false;
        assert_eq!(on_window_close(&policy), WindowClose::PauseReadable);
        let restored = reopen(&policy).unwrap();
        assert_eq!(restored, policy);
        assert_eq!(
            on_window_close(&SessionPolicy::default()),
            WindowClose::Stop
        );
    }
}
