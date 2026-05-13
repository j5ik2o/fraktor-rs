//! Journal actor configuration.

/// Configuration for journal actor retry behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalActorConfig {
  retry_max: u32,
}

impl JournalActorConfig {
  /// Creates a new configuration with the provided retry limit.
  #[must_use]
  pub const fn new(retry_max: u32) -> Self {
    Self { retry_max }
  }

  /// Returns the maximum number of retry polls allowed.
  #[must_use]
  pub const fn retry_max(&self) -> u32 {
    self.retry_max
  }

  /// Updates the retry limit.
  #[must_use]
  pub const fn with_retry_max(mut self, retry_max: u32) -> Self {
    self.retry_max = retry_max;
    self
  }

  pub(crate) const fn default_config() -> Self {
    Self { retry_max: 1 }
  }
}

impl Default for JournalActorConfig {
  fn default() -> Self {
    Self::default_config()
  }
}
