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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn priority_backend_config_new() {
    let config = PriorityBackendConfig::new(10, 0, 5, 2);
    assert_eq!(config.capacity(), 10);
    assert_eq!(config.min_priority(), 0);
    assert_eq!(config.max_priority(), 5);
    assert_eq!(config.default_priority(), 2);
  }

  #[test]
  fn priority_backend_config_with_default_layout() {
    let config = PriorityBackendConfig::with_default_layout(20);
    assert_eq!(config.capacity(), 20);
    assert_eq!(config.min_priority(), 0);
    assert_eq!(config.max_priority(), (PRIORITY_LEVELS - 1) as i8);
    assert_eq!(config.default_priority(), DEFAULT_PRIORITY);
  }

  #[test]
  fn priority_backend_config_clamp_priority_within_range() {
    let config = PriorityBackendConfig::new(10, 0, 5, 2);
    assert_eq!(config.clamp_priority(3), 3);
  }

  #[test]
  fn priority_backend_config_clamp_priority_below_min() {
    let config = PriorityBackendConfig::new(10, 0, 5, 2);
    assert_eq!(config.clamp_priority(-1), 0);
  }

  #[test]
  fn priority_backend_config_clamp_priority_above_max() {
    let config = PriorityBackendConfig::new(10, 0, 5, 2);
    assert_eq!(config.clamp_priority(10), 5);
  }

  #[test]
  #[should_panic(expected = "min_priority must not exceed max_priority")]
  fn priority_backend_config_new_panics_on_invalid_range() {
    PriorityBackendConfig::new(10, 5, 0, 2);
  }

  #[test]
  #[should_panic(expected = "default_priority must be within bounds")]
  fn priority_backend_config_new_panics_on_default_below_min() {
    PriorityBackendConfig::new(10, 0, 5, -1);
  }

  #[test]
  #[should_panic(expected = "default_priority must be within bounds")]
  fn priority_backend_config_new_panics_on_default_above_max() {
    PriorityBackendConfig::new(10, 0, 5, 10);
  }

  #[test]
  fn priority_backend_config_queue_storage_capacity() {
    let config = PriorityBackendConfig::new(15, 0, 5, 2);
    let storage: &dyn QueueStorage<i32> = &config;
    assert_eq!(storage.capacity(), 15);
  }

  #[test]
  fn priority_backend_config_queue_storage_read_unchecked_returns_null() {
    let config = PriorityBackendConfig::new(10, 0, 5, 2);
    let storage: &dyn QueueStorage<i32> = &config;
    let ptr = unsafe { storage.read_unchecked(0) };
    assert!(ptr.is_null());
  }

  #[test]
  #[should_panic(expected = "PriorityBackendConfig では write_unchecked を呼び出せません")]
  fn priority_backend_config_queue_storage_write_unchecked_panics() {
    let mut config = PriorityBackendConfig::new(10, 0, 5, 2);
    let storage: &mut dyn QueueStorage<i32> = &mut config;
    unsafe {
      storage.write_unchecked(0, 42);
    }
  }

  #[test]
  fn priority_backend_config_accessors() {
    let config = PriorityBackendConfig::new(100, -10, 10, 0);
    assert_eq!(config.capacity(), 100);
    assert_eq!(config.min_priority(), -10);
    assert_eq!(config.max_priority(), 10);
    assert_eq!(config.default_priority(), 0);
  }
}
