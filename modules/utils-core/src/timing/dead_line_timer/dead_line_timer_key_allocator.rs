#[cfg(not(target_has_atomic = "ptr"))]
use core::cell::Cell;
#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(target_has_atomic = "ptr"))]
use critical_section::with;

use super::dead_line_timer_key::DeadLineTimerKey;

/// Allocator for generating [`DeadLineTimerKey`] values.
#[derive(Debug)]
pub struct DeadLineTimerKeyAllocator {
  #[cfg(target_has_atomic = "ptr")]
  counter: AtomicUsize,
  #[cfg(not(target_has_atomic = "ptr"))]
  counter: Cell<usize>,
}

impl DeadLineTimerKeyAllocator {
  /// Creates a new allocator.
  #[must_use]
  #[inline]
  pub const fn new() -> Self {
    #[cfg(target_has_atomic = "ptr")]
    {
      Self { counter: AtomicUsize::new(1) }
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      Self { counter: Cell::new(1) }
    }
  }

  /// Issues a new unique key.
  #[inline]
  pub fn allocate(&self) -> DeadLineTimerKey {
    #[cfg(target_has_atomic = "ptr")]
    {
      let next = self.counter.fetch_add(1, Ordering::Relaxed) as u64;
      let raw = if next == 0 { 1 } else { next };
      DeadLineTimerKey::from_raw(raw)
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      let issued = with(|_| {
        let current = self.counter.get();
        let next = current.wrapping_add(1);
        let stored = if next == 0 { 1 } else { next };
        self.counter.set(stored);
        if current == 0 { 1 } else { current }
      });
      DeadLineTimerKey::from_raw(issued as u64)
    }
  }

  /// Checks the next key to be issued (for testing purposes).
  #[inline]
  pub fn peek(&self) -> DeadLineTimerKey {
    #[cfg(target_has_atomic = "ptr")]
    {
      DeadLineTimerKey::from_raw(self.counter.load(Ordering::Relaxed) as u64)
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      with(|_| DeadLineTimerKey::from_raw(self.counter.get() as u64))
    }
  }
}

impl Default for DeadLineTimerKeyAllocator {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn deadline_timer_key_allocator_new() {
    let allocator = DeadLineTimerKeyAllocator::new();
    let first_key = allocator.peek();
    assert!(first_key.is_valid());
    assert_eq!(first_key.into_raw(), 1);
  }

  #[test]
  fn deadline_timer_key_allocator_default() {
    let allocator = DeadLineTimerKeyAllocator::default();
    let first_key = allocator.peek();
    assert_eq!(first_key.into_raw(), 1);
  }

  #[test]
  fn deadline_timer_key_allocator_allocate() {
    let allocator = DeadLineTimerKeyAllocator::new();
    let key1 = allocator.allocate();
    let key2 = allocator.allocate();
    let key3 = allocator.allocate();

    assert!(key1.is_valid());
    assert!(key2.is_valid());
    assert!(key3.is_valid());

    assert_eq!(key1.into_raw(), 1);
    assert_eq!(key2.into_raw(), 2);
    assert_eq!(key3.into_raw(), 3);
  }

  #[test]
  fn deadline_timer_key_allocator_peek() {
    let allocator = DeadLineTimerKeyAllocator::new();
    let initial = allocator.peek();
    assert_eq!(initial.into_raw(), 1);

    let _key1 = allocator.allocate();
    let after_one = allocator.peek();
    assert_eq!(after_one.into_raw(), 2);

    let _key2 = allocator.allocate();
    let after_two = allocator.peek();
    assert_eq!(after_two.into_raw(), 3);
  }

  #[test]
  fn deadline_timer_key_allocator_unique_keys() {
    let allocator = DeadLineTimerKeyAllocator::new();
    let keys: Vec<_> = (0..10).map(|_| allocator.allocate()).collect();

    for (i, key) in keys.iter().enumerate() {
      assert_eq!(key.into_raw(), (i + 1) as u64);
    }
  }

  #[test]
  fn deadline_timer_key_allocator_debug() {
    let allocator = DeadLineTimerKeyAllocator::new();
    let debug_str = format!("{:?}", allocator);
    assert!(debug_str.contains("DeadLineTimerKeyAllocator"));
  }
}
