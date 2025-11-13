//! Scheduler configuration shared by actor systems.

use core::time::Duration;

use fraktor_utils_core_rs::time::SchedulerCapacityProfile;

/// Configuration for scheduler construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerConfig {
  resolution:       Duration,
  profile:          SchedulerCapacityProfile,
  max_pending_jobs: usize,
}

impl SchedulerConfig {
  /// Creates a configuration with the specified tick resolution and capacity profile.
  #[must_use]
  pub const fn new(resolution: Duration, profile: SchedulerCapacityProfile) -> Self {
    Self { resolution, profile, max_pending_jobs: profile.system_quota() }
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

  /// Overrides the maximum number of pending timer jobs accepted before applying backpressure.
  #[must_use]
  pub fn with_max_pending_jobs(mut self, max_pending_jobs: usize) -> Self {
    self.max_pending_jobs = max_pending_jobs.max(1);
    self
  }

  /// Returns the configured pending job limit.
  #[must_use]
  pub const fn max_pending_jobs(&self) -> usize {
    self.max_pending_jobs
  }
}

impl Default for SchedulerConfig {
  fn default() -> Self {
    Self::new(Duration::from_millis(10), SchedulerCapacityProfile::standard())
  }
}
