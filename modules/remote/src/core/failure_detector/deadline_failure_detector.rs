//! Deadline-based failure detector for a single monitored resource.

mod config;
#[cfg(test)]
mod tests;

pub use config::DeadlineFailureDetectorConfig;

use super::failure_detector::FailureDetector;

/// Failure detector using an absolute timeout of missing heartbeats.
///
/// The resource is considered unavailable when no heartbeat arrives within
/// `heartbeat_interval + acceptable_heartbeat_pause` milliseconds.
pub struct DeadlineFailureDetector {
  deadline_ms:         u64,
  heartbeat_timestamp: Option<u64>,
}

impl DeadlineFailureDetector {
  /// Creates a detector with the provided configuration.
  #[must_use]
  pub fn new(config: DeadlineFailureDetectorConfig) -> Self {
    Self { deadline_ms: config.deadline_ms(), heartbeat_timestamp: None }
  }
}

impl FailureDetector for DeadlineFailureDetector {
  fn is_available(&self, now_ms: u64) -> bool {
    match self.heartbeat_timestamp {
      // Pekko 互換: ハートビート未受信のリソースは健全として扱う
      | None => true,
      | Some(ts) => (ts + self.deadline_ms) > now_ms,
    }
  }

  fn is_monitoring(&self) -> bool {
    self.heartbeat_timestamp.is_some()
  }

  fn heartbeat(&mut self, now_ms: u64) {
    self.heartbeat_timestamp = Some(now_ms);
  }
}
