//! Scheduler configuration shared by actor systems.

use core::time::Duration;

use fraktor_utils_core_rs::time::SchedulerCapacityProfile;

/// Configuration for scheduler construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerConfig {
  resolution: Duration,
  profile:    SchedulerCapacityProfile,
}

impl SchedulerConfig {
  /// Creates a configuration with the specified tick resolution and capacity profile.
  #[must_use]
  pub const fn new(resolution: Duration, profile: SchedulerCapacityProfile) -> Self {
    Self { resolution, profile }
  }

  /// Returns the configured resolution.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Returns the capacity profile.
  #[must_use]
  pub const fn profile(&self) -> SchedulerCapacityProfile {
    self.profile
  }
}

impl Default for SchedulerConfig {
  fn default() -> Self {
    Self::new(Duration::from_millis(10), SchedulerCapacityProfile::standard())
  }
}
