use super::SubscriptionTimeoutMode;

/// Configuration for subscription timeout behavior.
///
/// Controls what happens when a stream subscription is not consumed
/// within a configured number of ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionTimeoutConfig {
  mode:          SubscriptionTimeoutMode,
  timeout_ticks: usize,
}

impl SubscriptionTimeoutConfig {
  /// Creates new subscription timeout config.
  #[must_use]
  pub const fn new(mode: SubscriptionTimeoutMode, timeout_ticks: usize) -> Self {
    Self { mode, timeout_ticks }
  }

  /// Returns the timeout mode.
  #[must_use]
  pub const fn mode(&self) -> SubscriptionTimeoutMode {
    self.mode
  }

  /// Returns the timeout duration in ticks.
  #[must_use]
  pub const fn timeout_ticks(&self) -> usize {
    self.timeout_ticks
  }
}

impl Default for SubscriptionTimeoutConfig {
  fn default() -> Self {
    Self { mode: SubscriptionTimeoutMode::Cancel, timeout_ticks: 5000 }
  }
}
