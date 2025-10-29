//! Dispatcher responsible for scheduling mailbox work with throughput limiting.

/// Dispatcher executes mailbox processing loops while enforcing throughput limits.
pub struct Dispatcher {
  throughput: u32,
}

impl Dispatcher {
  /// Creates a dispatcher with the specified throughput per turn.
  #[must_use]
  pub fn new(throughput: u32) -> Self {
    Self { throughput: throughput.max(1) }
  }

  /// Returns the configured throughput.
  #[must_use]
  pub fn throughput(&self) -> u32 {
    self.throughput
  }

  /// Runs the supplied closure until the throughput budget is exhausted or the closure returns
  /// `false` to indicate no more work is pending.
  pub fn dispatch<F>(&self, mut process_one: F)
  where
    F: FnMut() -> bool, {
    let mut remaining = self.throughput;
    while remaining > 0 {
      if !process_one() {
        break;
      }
      remaining -= 1;
    }
  }
}

impl Default for Dispatcher {
  fn default() -> Self {
    Self::new(300)
  }
}
