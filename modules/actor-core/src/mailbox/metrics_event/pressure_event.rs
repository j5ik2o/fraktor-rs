use core::time::Duration;

use crate::actor_prim::Pid;

/// Event describing high mailbox utilisation.
#[derive(Clone, Debug)]
pub struct MailboxPressureEvent {
  pid:         Pid,
  user_len:    usize,
  capacity:    usize,
  utilization: u8,
  timestamp:   Duration,
  threshold:   Option<usize>,
}

impl MailboxPressureEvent {
  /// Creates a new pressure event using utilisation percentage.
  #[must_use]
  pub const fn new(
    pid: Pid,
    user_len: usize,
    capacity: usize,
    utilization: u8,
    timestamp: Duration,
    threshold: Option<usize>,
  ) -> Self {
    Self { pid, user_len, capacity, utilization, timestamp, threshold }
  }

  /// Returns the owning actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the queued user messages.
  #[must_use]
  pub const fn user_len(&self) -> usize {
    self.user_len
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the utilisation percentage (0-100).
  #[must_use]
  pub const fn utilization(&self) -> u8 {
    self.utilization
  }

  /// Returns the optional warning threshold associated with the mailbox.
  #[must_use]
  pub const fn threshold(&self) -> Option<usize> {
    self.threshold
  }

  /// Returns the timestamp when the event was emitted.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}
