//! Event describing mailbox utilisation metrics.

use core::time::Duration;

use crate::actor_prim::Pid;

/// Snapshot of mailbox queue lengths and capacity.
#[derive(Clone, Debug)]
pub struct MailboxMetricsEvent {
  pid:        Pid,
  user_len:   usize,
  system_len: usize,
  capacity:   Option<usize>,
  throughput: Option<usize>,
  timestamp:  Duration,
}

/// Event describing high mailbox utilisation.
#[derive(Clone, Debug)]
pub struct MailboxPressureEvent {
  pid:         Pid,
  user_len:    usize,
  capacity:    usize,
  utilization: u8,
  timestamp:   Duration,
}

impl MailboxPressureEvent {
  /// Creates a new pressure event using utilisation percentage.
  #[must_use]
  pub const fn new(pid: Pid, user_len: usize, capacity: usize, utilization: u8, timestamp: Duration) -> Self {
    Self { pid, user_len, capacity, utilization, timestamp }
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

  /// Returns the timestamp when the event was emitted.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}

impl MailboxMetricsEvent {
  /// Creates a new mailbox metrics event.
  #[must_use]
  pub const fn new(
    pid: Pid,
    user_len: usize,
    system_len: usize,
    capacity: Option<usize>,
    throughput: Option<usize>,
    timestamp: Duration,
  ) -> Self {
    Self { pid, user_len, system_len, capacity, throughput, timestamp }
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

  /// Returns the queued system messages.
  #[must_use]
  pub const fn system_len(&self) -> usize {
    self.system_len
  }

  /// Returns the configured capacity if bounded.
  #[must_use]
  pub const fn capacity(&self) -> Option<usize> {
    self.capacity
  }

  /// Returns the configured throughput limit.
  #[must_use]
  pub const fn throughput(&self) -> Option<usize> {
    self.throughput
  }

  /// Returns the timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}
