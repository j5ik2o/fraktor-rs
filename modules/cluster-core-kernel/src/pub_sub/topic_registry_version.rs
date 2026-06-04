//! Monotonic version for pub-sub registry buckets and entries.

/// Monotonic pub-sub registry version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopicRegistryVersion(u64);

impl TopicRegistryVersion {
  /// Returns the zero version.
  #[must_use]
  pub const fn zero() -> Self {
    Self(0)
  }

  /// Creates a version from a raw value.
  #[must_use]
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Returns the raw version value.
  #[must_use]
  pub const fn value(self) -> u64 {
    self.0
  }

  /// Returns the next version.
  #[must_use]
  pub const fn next(self) -> Self {
    Self(self.0 + 1)
  }
}

impl Default for TopicRegistryVersion {
  fn default() -> Self {
    Self::zero()
  }
}
