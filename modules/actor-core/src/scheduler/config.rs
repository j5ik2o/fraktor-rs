//! Scheduler configuration shared by actor systems.

use core::{num::NonZeroU32, time::Duration};

use fraktor_utils_core_rs::time::SchedulerCapacityProfile;

use super::{
  fixed_delay_policy::FixedDelayPolicy,
  fixed_rate_policy::FixedRatePolicy,
  policy_registry::SchedulerPolicyRegistry,
};

/// Configuration for scheduler construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerConfig {
  resolution:       Duration,
  profile:          SchedulerCapacityProfile,
  max_pending_jobs: usize,
  policy_registry:  SchedulerPolicyRegistry,
  task_run_capacity: usize,
}

impl SchedulerConfig {
  /// Creates a configuration with the specified tick resolution and capacity profile.
  #[must_use]
  pub fn new(resolution: Duration, profile: SchedulerCapacityProfile) -> Self {
    let max_pending_jobs = profile.system_quota();
    let task_run_capacity = profile.task_run_capacity();
    Self { resolution, profile, max_pending_jobs, policy_registry: SchedulerPolicyRegistry::default(), task_run_capacity }
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

  /// Configures the backlog limit for periodic jobs.
  #[must_use]
  pub fn with_backlog_limit(mut self, backlog_limit: u32) -> Self {
    let limit = clamp_non_zero(backlog_limit);
    let rate = self.policy_registry.fixed_rate().with_backlog_limit(limit);
    let delay = self.policy_registry.fixed_delay().with_backlog_limit(limit);
    self.policy_registry = SchedulerPolicyRegistry::new(rate, delay);
    self
  }

  /// Returns the backlog limit.
  #[must_use]
  pub const fn backlog_limit(&self) -> u32 {
    self.policy_registry.fixed_rate().backlog_limit().get()
  }

  /// Configures the burst warning threshold.
  #[must_use]
  pub fn with_burst_threshold(mut self, burst_threshold: u32) -> Self {
    let threshold = clamp_non_zero(burst_threshold);
    let rate = self.policy_registry.fixed_rate().with_burst_threshold(threshold);
    let delay = self.policy_registry.fixed_delay().with_burst_threshold(threshold);
    self.policy_registry = SchedulerPolicyRegistry::new(rate, delay);
    self
  }

  /// Returns the burst warning threshold.
  #[must_use]
  pub const fn burst_threshold(&self) -> u32 {
    self.policy_registry.fixed_rate().burst_threshold().get()
  }

  /// Overrides only the fixed-rate policy.
  #[must_use]
  pub fn with_fixed_rate_policy(mut self, policy: FixedRatePolicy) -> Self {
    self.policy_registry = self.policy_registry.with_fixed_rate(policy);
    self
  }

  /// Overrides only the fixed-delay policy.
  #[must_use]
  pub fn with_fixed_delay_policy(mut self, policy: FixedDelayPolicy) -> Self {
    self.policy_registry = self.policy_registry.with_fixed_delay(policy);
    self
  }

  /// Replaces the entire policy registry.
  #[must_use]
  pub fn with_policy_registry(mut self, registry: SchedulerPolicyRegistry) -> Self {
    self.policy_registry = registry;
    self
  }

  /// Returns the full policy registry.
  #[must_use]
  pub const fn policy_registry(&self) -> SchedulerPolicyRegistry {
    self.policy_registry
  }

  /// Returns the fixed-rate policy.
  #[must_use]
  pub const fn fixed_rate_policy(&self) -> FixedRatePolicy {
    self.policy_registry.fixed_rate()
  }

  /// Returns the fixed-delay policy.
  #[must_use]
  pub const fn fixed_delay_policy(&self) -> FixedDelayPolicy {
    self.policy_registry.fixed_delay()
  }

  /// Returns the capacity allocated for TaskRunOnClose registrations.
  #[must_use]
  pub const fn task_run_capacity(&self) -> usize {
    self.task_run_capacity
  }

  /// Overrides the task run capacity.
  #[must_use]
  pub fn with_task_run_capacity(mut self, capacity: usize) -> Self {
    self.task_run_capacity = capacity.max(1);
    self
  }
}

impl Default for SchedulerConfig {
  fn default() -> Self {
    Self::new(Duration::from_millis(10), SchedulerCapacityProfile::standard())
  }
}

fn clamp_non_zero(value: u32) -> NonZeroU32 {
  NonZeroU32::new(value.max(1)).expect("non-zero backlog value")
}
