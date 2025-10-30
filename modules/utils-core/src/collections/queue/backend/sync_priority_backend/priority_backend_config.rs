use crate::collections::{DEFAULT_PRIORITY, PRIORITY_LEVELS, queue::QueueStorage};

/// Configuration object passed into `SyncQueueBackend` implementations.
pub struct PriorityBackendConfig {
  capacity:         usize,
  min_priority:     i8,
  max_priority:     i8,
  default_priority: i8,
}

impl PriorityBackendConfig {
  /// Creates a new configuration with explicit bounds.
  ///
  /// # Panics
  ///
  /// Panics if `min_priority > max_priority` or if `default_priority` is not within the range
  /// `[min_priority, max_priority]`.
  #[must_use]
  pub fn new(capacity: usize, min_priority: i8, max_priority: i8, default_priority: i8) -> Self {
    assert!(min_priority <= max_priority, "min_priority must not exceed max_priority");
    assert!(
      default_priority >= min_priority && default_priority <= max_priority,
      "default_priority must be within bounds"
    );
    Self { capacity, min_priority, max_priority, default_priority }
  }

  /// Creates a configuration using the project default priority layout.
  #[must_use]
  pub fn with_default_layout(capacity: usize) -> Self {
    let levels = PRIORITY_LEVELS as i8;
    let max_priority = levels.saturating_sub(1);
    let min_priority = 0;
    Self::new(capacity, min_priority, max_priority, DEFAULT_PRIORITY)
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the minimum priority value.
  #[must_use]
  pub const fn min_priority(&self) -> i8 {
    self.min_priority
  }

  /// Returns the maximum priority value.
  #[must_use]
  pub const fn max_priority(&self) -> i8 {
    self.max_priority
  }

  /// Returns the default priority value applied when a message omits an explicit priority.
  #[must_use]
  pub const fn default_priority(&self) -> i8 {
    self.default_priority
  }

  /// Clamps the provided priority value into the configured bounds.
  #[must_use]
  pub fn clamp_priority(&self, value: i8) -> i8 {
    value.clamp(self.min_priority, self.max_priority)
  }
}

impl<T> QueueStorage<T> for PriorityBackendConfig {
  fn capacity(&self) -> usize {
    self.capacity
  }

  unsafe fn read_unchecked(&self, _idx: usize) -> *const T {
    core::ptr::null()
  }

  unsafe fn write_unchecked(&mut self, _idx: usize, value: T) {
    drop(value);
    panic!("PriorityBackendConfig では write_unchecked を呼び出せません");
  }
}
