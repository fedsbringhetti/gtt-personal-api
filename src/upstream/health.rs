use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::UpstreamError;

/// Outcome of the most recent upstream calls, shared by everything that uses the client.
#[derive(Debug, Default)]
pub struct UpstreamHealth {
    state: Mutex<HealthSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HealthSnapshot {
    /// `None` until the first call.
    pub ok: Option<bool>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

impl UpstreamHealth {
    pub fn snapshot(&self) -> HealthSnapshot {
        self.lock().clone()
    }

    pub fn record_success(&self, at: DateTime<Utc>) {
        let mut state = self.lock();
        state.ok = Some(true);
        state.last_success = Some(at);
        state.consecutive_failures = 0;
    }

    pub fn record_failure(&self, at: DateTime<Utc>, err: &UpstreamError) {
        let mut state = self.lock();
        state.ok = Some(false);
        state.last_failure = Some(at);
        state.last_error = Some(err.to_string());
        state.consecutive_failures += 1;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HealthSnapshot> {
        // A panic while holding the lock cannot leave the snapshot inconsistent.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_successes_and_consecutive_failures() {
        let health = UpstreamHealth::default();
        assert_eq!(health.snapshot().ok, None);

        let t1 = DateTime::from_timestamp(1, 0).unwrap();
        let t2 = DateTime::from_timestamp(2, 0).unwrap();
        health.record_failure(t1, &UpstreamError::Timeout);
        health.record_failure(t2, &UpstreamError::Status(503));
        let state = health.snapshot();
        assert_eq!(state.ok, Some(false));
        assert_eq!(state.consecutive_failures, 2);
        assert_eq!(state.last_failure, Some(t2));
        assert_eq!(
            state.last_error.as_deref(),
            Some("upstream returned HTTP 503")
        );

        health.record_success(t2);
        let state = health.snapshot();
        assert_eq!(state.ok, Some(true));
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.last_success, Some(t2));
        assert_eq!(state.last_failure, Some(t2), "kept for diagnosis");
    }
}
