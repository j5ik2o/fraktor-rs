//! Registry storing scheduler policies per periodic mode.

use super::{fixed_delay_policy::FixedDelayPolicy, fixed_rate_policy::FixedRatePolicy};

/// Aggregates policy defaults for scheduler jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerPolicyRegistry {
  fixed_rate:  FixedRatePolicy,
  fixed_delay: FixedDelayPolicy,
}

impl SchedulerPolicyRegistry {
  /// Creates a registry with the provided policies.
  #[must_use]
  pub const fn new(fixed_rate: FixedRatePolicy, fixed_delay: FixedDelayPolicy) -> Self {
    Self { fixed_rate, fixed_delay }
  }

  /// Returns the fixed-rate policy.
  #[must_use]
  pub const fn fixed_rate(&self) -> FixedRatePolicy {
    self.fixed_rate
  }

  /// Returns the fixed-delay policy.
  #[must_use]
  pub const fn fixed_delay(&self) -> FixedDelayPolicy {
    self.fixed_delay
  }

  /// Replaces the fixed-rate policy.
  #[must_use]
  pub const fn with_fixed_rate(mut self, policy: FixedRatePolicy) -> Self {
    self.fixed_rate = policy;
    self
  }

  /// Replaces the fixed-delay policy.
  #[must_use]
  pub const fn with_fixed_delay(mut self, policy: FixedDelayPolicy) -> Self {
    self.fixed_delay = policy;
    self
  }
}

impl Default for SchedulerPolicyRegistry {
  fn default() -> Self {
    Self::new(FixedRatePolicy::default(), FixedDelayPolicy::default())
  }
}
