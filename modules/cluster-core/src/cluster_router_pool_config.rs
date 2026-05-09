//! Minimal config for cluster router pool behavior.

#[cfg(test)]
mod tests;

/// Config for pool-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterPoolConfig {
  total_instances:     usize,
  allow_local_routees: bool,
}

impl ClusterRouterPoolConfig {
  /// Creates config with the provided total instance count.
  ///
  /// # Panics
  ///
  /// Panics when `total_instances` is zero.
  #[must_use]
  pub fn new(total_instances: usize) -> Self {
    assert!(total_instances > 0, "total instances must be > 0");
    Self { total_instances, allow_local_routees: true }
  }

  /// Overrides whether local routees are allowed.
  #[must_use]
  pub const fn with_allow_local_routees(mut self, allow: bool) -> Self {
    self.allow_local_routees = allow;
    self
  }

  /// Returns the configured total instance count.
  #[must_use]
  pub const fn total_instances(&self) -> usize {
    self.total_instances
  }

  /// Returns whether local routees are allowed.
  #[must_use]
  pub const fn allow_local_routees(&self) -> bool {
    self.allow_local_routees
  }
}
