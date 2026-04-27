//! Deadline-window restart counter for reconnect loops.

use core::time::Duration;

/// Counts restart attempts within a bounded timeout window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestartCounter {
  max_restarts:       u32,
  restart_timeout_ms: u64,
  count:              u32,
  deadline_ms:        u64,
}

impl RestartCounter {
  /// Creates a counter from a restart budget and timeout window.
  #[must_use]
  pub fn new(max_restarts: u32, restart_timeout: Duration) -> Self {
    Self { max_restarts, restart_timeout_ms: duration_millis(restart_timeout), count: 0, deadline_ms: 0 }
  }

  /// Returns the number of restarts in the current timeout window.
  #[must_use]
  pub const fn count(&self) -> u32 {
    self.count
  }

  /// Records one restart attempt at `now_ms` and returns whether it is allowed.
  #[must_use]
  pub fn restart(&mut self, now_ms: u64) -> bool {
    if self.count > 0 && now_ms < self.deadline_ms {
      self.count = self.count.saturating_add(1);
    } else {
      self.count = 1;
      self.deadline_ms = now_ms.saturating_add(self.restart_timeout_ms);
    }
    self.count <= self.max_restarts
  }
}

fn duration_millis(duration: Duration) -> u64 {
  duration.as_millis().min(u128::from(u64::MAX)) as u64
}
