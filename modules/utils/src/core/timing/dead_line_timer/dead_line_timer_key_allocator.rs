#[cfg(target_has_atomic = "64")]
use portable_atomic::{AtomicU64 as AtomicCounter, Ordering};
#[cfg(target_has_atomic = "64")]
type CounterPrimitive = u64;

#[cfg(not(target_has_atomic = "64"))]
use portable_atomic::{AtomicU32 as AtomicCounter, Ordering};
#[cfg(not(target_has_atomic = "64"))]
type CounterPrimitive = u32;

use super::dead_line_timer_key::DeadLineTimerKey;

#[cfg(test)]
mod tests;

/// Allocates unique DeadlineTimer keys.
#[derive(Default)]
pub struct DeadLineTimerKeyAllocator {
  counter: AtomicCounter,
}

impl DeadLineTimerKeyAllocator {
  /// Creates a new allocator.
  #[must_use]
  pub const fn new() -> Self {
    Self { counter: AtomicCounter::new(Self::seed()) }
  }

  const fn seed() -> CounterPrimitive {
    let invalid = DeadLineTimerKey::invalid().into_raw();
    invalid.wrapping_add(1) as CounterPrimitive
  }

  /// Allocates a unique key.
  #[must_use]
  pub fn allocate(&self) -> DeadLineTimerKey {
    let raw = Self::into_u64(self.counter.fetch_add(1, Ordering::Relaxed));
    DeadLineTimerKey::from_raw(raw)
  }

  #[cfg(target_has_atomic = "64")]
  const fn into_u64(value: CounterPrimitive) -> u64 {
    value
  }

  #[cfg(not(target_has_atomic = "64"))]
  const fn into_u64(value: CounterPrimitive) -> u64 {
    value as u64
  }
}
