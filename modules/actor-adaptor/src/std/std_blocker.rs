//! Standard-library based [`Blocker`] implementation.

extern crate std;

use std::{
  sync::{Condvar, Mutex},
  time::Duration,
};

use fraktor_actor_rs::core::kernel::system::Blocker;

/// [`Blocker`] implementation using [`Condvar`] with periodic timeout wakeups.
///
/// Blocks the calling thread with minimal CPU usage by sleeping between
/// condition checks. The default poll interval is 1 ms, which provides a
/// good balance between latency and efficiency for system termination.
pub struct StdBlocker {
  poll_interval: Duration,
  pair:          (Mutex<()>, Condvar),
}

impl StdBlocker {
  /// Creates a blocker with the default 1 ms poll interval.
  #[must_use]
  pub fn new() -> Self {
    Self::with_poll_interval(Duration::from_millis(1))
  }

  /// Creates a blocker with a custom poll interval.
  #[must_use]
  pub fn with_poll_interval(poll_interval: Duration) -> Self {
    Self { poll_interval, pair: (Mutex::new(()), Condvar::new()) }
  }
}

impl Default for StdBlocker {
  fn default() -> Self {
    Self::new()
  }
}

impl Blocker for StdBlocker {
  fn block_until(&self, condition: &dyn Fn() -> bool) {
    if condition() {
      return;
    }
    let (lock, cvar) = &self.pair;
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    while !condition() {
      let (g, _) = cvar.wait_timeout(guard, self.poll_interval).unwrap_or_else(|poisoned| poisoned.into_inner());
      guard = g;
    }
  }
}
