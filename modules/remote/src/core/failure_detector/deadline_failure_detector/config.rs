/// Configuration for the deadline-based failure detector.
#[derive(Clone, Debug)]
pub struct DeadlineFailureDetectorConfig {
  acceptable_heartbeat_pause_ms: u64,
  heartbeat_interval_ms:         u64,
}

impl DeadlineFailureDetectorConfig {
  /// Creates a new configuration.
  ///
  /// # Panics
  ///
  /// Panics if `heartbeat_interval_ms` is zero.
  #[must_use]
  pub fn new(acceptable_heartbeat_pause_ms: u64, heartbeat_interval_ms: u64) -> Self {
    assert!(heartbeat_interval_ms > 0, "heartbeat_interval_ms must be > 0");
    Self { acceptable_heartbeat_pause_ms, heartbeat_interval_ms }
  }

  /// Returns the acceptable heartbeat pause in milliseconds.
  #[must_use]
  pub const fn acceptable_heartbeat_pause_ms(&self) -> u64 {
    self.acceptable_heartbeat_pause_ms
  }

  /// Returns the expected heartbeat interval in milliseconds.
  #[must_use]
  pub const fn heartbeat_interval_ms(&self) -> u64 {
    self.heartbeat_interval_ms
  }

  /// Returns the deadline in milliseconds (pause + interval).
  #[must_use]
  pub const fn deadline_ms(&self) -> u64 {
    self.acceptable_heartbeat_pause_ms + self.heartbeat_interval_ms
  }
}
