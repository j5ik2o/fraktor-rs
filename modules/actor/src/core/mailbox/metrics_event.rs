//! Event describing mailbox utilisation metrics.

use core::time::Duration;

mod pressure_event;

pub use pressure_event::MailboxPressureEvent;

use crate::core::actor_prim::Pid;

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
