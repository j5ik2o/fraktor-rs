//! Minimal config for cluster router pool behavior.

#[cfg(test)]
#[path = "cluster_router_pool_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

/// Config for pool-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterPoolConfig {
  total_instances:        usize,
  max_instances_per_node: usize,
  allow_local_routees:    bool,
  use_roles:              Vec<String>,
}

impl ClusterRouterPoolConfig {
  /// Creates config with the provided total instance count.
  ///
  /// Defaults to one routee instance per node and no role restriction.
  ///
  /// # Panics
  ///
  /// Panics when `total_instances` is zero.
  #[must_use]
  pub fn new(total_instances: usize) -> Self {
    assert!(total_instances > 0, "total instances must be > 0");
    Self { total_instances, max_instances_per_node: 1, allow_local_routees: true, use_roles: Vec::new() }
  }

  /// Overrides the maximum number of routee instances allowed on a single node.
  ///
  /// # Panics
  ///
  /// Panics when `max_instances_per_node` is zero.
  #[must_use]
  pub fn with_max_instances_per_node(mut self, max_instances_per_node: usize) -> Self {
    assert!(max_instances_per_node > 0, "max instances per node must be > 0");
    self.max_instances_per_node = max_instances_per_node;
    self
  }

  /// Overrides whether local routees are allowed.
  #[must_use]
  pub const fn with_allow_local_routees(mut self, allow: bool) -> Self {
    self.allow_local_routees = allow;
    self
  }

  /// Restricts routee placement to nodes that carry all of the given roles.
  #[must_use]
  pub fn with_use_roles(mut self, use_roles: Vec<String>) -> Self {
    self.use_roles = use_roles;
    self
  }

  /// Returns the configured total instance count.
  #[must_use]
  pub const fn total_instances(&self) -> usize {
    self.total_instances
  }

  /// Returns the maximum number of routee instances allowed on a single node.
  #[must_use]
  pub const fn max_instances_per_node(&self) -> usize {
    self.max_instances_per_node
  }

  /// Returns whether local routees are allowed.
  #[must_use]
  pub const fn allow_local_routees(&self) -> bool {
    self.allow_local_routees
  }

  /// Returns the roles a node must carry to host routees.
  #[must_use]
  pub fn use_roles(&self) -> &[String] {
    &self.use_roles
  }

  /// Returns whether a node carrying `member_roles` is allowed to host routees.
  ///
  /// A node qualifies when it carries every configured role; an empty role
  /// requirement matches every node.
  #[must_use]
  pub fn satisfies_roles(&self, member_roles: &[String]) -> bool {
    self.use_roles.iter().all(|role| member_roles.contains(role))
  }
}
