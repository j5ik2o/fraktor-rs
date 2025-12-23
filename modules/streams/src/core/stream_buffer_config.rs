use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

/// Configuration for stream buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamBufferConfig {
  capacity:        usize,
  overflow_policy: OverflowPolicy,
}

impl StreamBufferConfig {
  /// Creates a new configuration with the provided capacity and policy.
  #[must_use]
  pub const fn new(capacity: usize, overflow_policy: OverflowPolicy) -> Self {
    Self { capacity, overflow_policy }
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the configured overflow policy.
  #[must_use]
  pub const fn overflow_policy(&self) -> OverflowPolicy {
    self.overflow_policy
  }

  /// Updates the capacity.
  #[must_use]
  pub const fn with_capacity(mut self, capacity: usize) -> Self {
    self.capacity = capacity;
    self
  }

  /// Updates the overflow policy.
  #[must_use]
  pub const fn with_overflow_policy(mut self, overflow_policy: OverflowPolicy) -> Self {
    self.overflow_policy = overflow_policy;
    self
  }
}

impl Default for StreamBufferConfig {
  fn default() -> Self {
    Self { capacity: 16, overflow_policy: OverflowPolicy::Block }
  }
}
