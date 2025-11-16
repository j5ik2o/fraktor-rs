//! Predefined capacity profiles for the scheduler.

#[cfg(test)]
mod tests;

/// Capacity settings shared between timer wheel, overflow pool, and on-close tasks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerCapacityProfile {
  name:              &'static str,
  system_quota:      usize,
  overflow_capacity: usize,
  task_run_capacity: usize,
}

impl SchedulerCapacityProfile {
  /// Cortex-M class profile (2,048 timers).
  #[must_use]
  pub const fn tiny() -> Self {
    Self::new("Tiny", 2_048, 512, 128)
  }

  /// Small RTOS profile (4,096 timers).
  #[must_use]
  pub const fn small() -> Self {
    Self::new("Small", 4_096, 1_024, 256)
  }

  /// Standard host profile (10,240 timers).
  #[must_use]
  pub const fn standard() -> Self {
    Self::new("Standard", 10_240, 2_560, 512)
  }

  /// Large control-plane profile (25,600 timers).
  #[must_use]
  pub const fn large() -> Self {
    Self::new("Large", 25_600, 6_400, 1_024)
  }

  /// Creates a custom profile.
  #[must_use]
  pub const fn new(
    name: &'static str,
    system_quota: usize,
    overflow_capacity: usize,
    task_run_capacity: usize,
  ) -> Self {
    Self { name, system_quota, overflow_capacity, task_run_capacity }
  }

  /// Human readable profile name.
  #[must_use]
  pub const fn name(&self) -> &'static str {
    self.name
  }

  /// Maximum active timers within the wheel.
  #[must_use]
  pub const fn system_quota(&self) -> usize {
    self.system_quota
  }

  /// Suggested tick buffer quota for scheduler driver feeds.
  #[must_use]
  pub const fn tick_buffer_quota(&self) -> usize {
    let base = self.system_quota / 8;
    if base < 32 { 32 } else { base }
  }

  /// Overflow queue capacity for far-future timers.
  #[must_use]
  pub const fn overflow_capacity(&self) -> usize {
    self.overflow_capacity
  }

  /// Overall capacity for on-close task registrations.
  #[must_use]
  pub const fn task_run_capacity(&self) -> usize {
    self.task_run_capacity
  }
}
