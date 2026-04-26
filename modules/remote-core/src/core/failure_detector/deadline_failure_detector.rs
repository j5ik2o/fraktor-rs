//! Deadline-based failure detector.

/// Failure detector using an absolute timeout of missing heartbeats.
#[derive(Debug, Clone)]
pub struct DeadlineFailureDetector {
  acceptable_heartbeat_pause_ms: u64,
  last_heartbeat_ms:             Option<u64>,
}

impl DeadlineFailureDetector {
  /// Creates a new deadline failure detector.
  #[must_use]
  pub const fn new(acceptable_heartbeat_pause_ms: u64) -> Self {
    Self { acceptable_heartbeat_pause_ms, last_heartbeat_ms: None }
  }

  /// Records a heartbeat at `now_ms` monotonic millis.
  pub const fn heartbeat(&mut self, now_ms: u64) {
    self.last_heartbeat_ms = Some(now_ms);
  }

  /// Returns `true` while the monitored resource is within its heartbeat deadline.
  #[must_use]
  pub const fn is_available(&self, now_ms: u64) -> bool {
    let Some(last_heartbeat_ms) = self.last_heartbeat_ms else {
      return true;
    };
    let deadline_ms = last_heartbeat_ms.saturating_add(self.acceptable_heartbeat_pause_ms);
    deadline_ms > now_ms
  }

  /// Returns `true` after at least one heartbeat has been recorded.
  #[must_use]
  pub const fn is_monitoring(&self) -> bool {
    self.last_heartbeat_ms.is_some()
  }
}
