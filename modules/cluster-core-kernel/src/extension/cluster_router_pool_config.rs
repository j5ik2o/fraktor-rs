//! Minimal config for cluster router pool behavior.

#[cfg(test)]
#[path = "cluster_router_pool_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

/// Config for pool-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterPoolConfig {
  total_instances:        usize,
  allow_local_routees:    bool,
  use_roles:              Vec<String>,
  max_instances_per_node: Option<usize>,
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
    Self { total_instances, allow_local_routees: true, use_roles: Vec::new(), max_instances_per_node: None }
  }

  /// Overrides whether local routees are allowed.
  #[must_use]
  pub const fn with_allow_local_routees(mut self, allow: bool) -> Self {
    self.allow_local_routees = allow;
    self
  }

  /// Restricts routee selection to members with any of the supplied roles.
  #[must_use]
  pub fn with_use_roles(mut self, roles: Vec<String>) -> Self {
    self.use_roles = normalize_roles(roles);
    self
  }

  /// Caps routee instances allocated to a single node.
  ///
  /// # Panics
  ///
  /// Panics when `max_instances_per_node` is zero.
  #[must_use]
  pub fn with_max_instances_per_node(mut self, max_instances_per_node: usize) -> Self {
    assert!(max_instances_per_node > 0, "max instances per node must be > 0");
    self.max_instances_per_node = Some(max_instances_per_node);
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

  /// Returns role constraints for routee selection.
  #[must_use]
  pub fn use_roles(&self) -> &[String] {
    &self.use_roles
  }

  /// Returns the per-node routee instance cap.
  #[must_use]
  pub const fn max_instances_per_node(&self) -> Option<usize> {
    self.max_instances_per_node
  }
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
