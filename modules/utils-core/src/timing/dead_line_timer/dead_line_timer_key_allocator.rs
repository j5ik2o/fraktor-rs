use core::sync::atomic::{AtomicU64, Ordering};

use super::dead_line_timer_key::DeadLineTimerKey;

#[cfg(test)]
mod tests;

/// Allocates unique DeadlineTimer keys.
#[derive(Default)]
pub struct DeadLineTimerKeyAllocator {
  counter: AtomicU64,
}

impl DeadLineTimerKeyAllocator {
  /// Creates a new allocator.
  #[must_use]
  pub const fn new() -> Self {
    Self { counter: AtomicU64::new(DeadLineTimerKey::invalid().into_raw() + 1) }
  }

  /// Allocates a unique key.
  #[must_use]
  pub fn allocate(&self) -> DeadLineTimerKey {
    DeadLineTimerKey::from_raw(self.counter.fetch_add(1, Ordering::Relaxed))
  }
}
