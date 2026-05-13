//! Configuration for timer wheel operation.

use core::time::Duration;

use super::SchedulerCapacityProfile;

/// Configuration shared by timer wheels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerWheelConfig {
  resolution:       Duration,
  slot_count:       u32,
  drift_budget_pct: u8,
}

impl TimerWheelConfig {
  /// Creates a configuration for tests.
  #[must_use]
  pub const fn new(resolution: Duration, slot_count: u32, drift_budget_pct: u8) -> Self {
    Self { resolution, slot_count, drift_budget_pct }
  }

  /// Creates configuration parameters from a capacity profile.
  #[must_use]
  pub const fn from_profile(profile: SchedulerCapacityProfile, resolution: Duration, drift_budget_pct: u8) -> Self {
    Self { resolution, slot_count: profile.system_quota() as u32, drift_budget_pct }
  }

  /// Tick resolution.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Number of slots maintained by the wheel.
  #[must_use]
  pub const fn slot_count(&self) -> u32 {
    self.slot_count
  }

  /// Allowed drift budget percentage.
  #[must_use]
  pub const fn drift_budget_pct(&self) -> u8 {
    self.drift_budget_pct
  }
}
