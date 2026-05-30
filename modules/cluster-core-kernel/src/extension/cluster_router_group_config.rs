//! Minimal config for cluster router group behavior.

#[cfg(test)]
#[path = "cluster_router_group_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

/// Config for group-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterGroupConfig {
  routee_paths:        Vec<String>,
  allow_local_routees: bool,
  use_roles:           Vec<String>,
}

impl ClusterRouterGroupConfig {
  /// Creates config with explicit routee paths.
  ///
  /// Defaults to no role restriction.
  #[must_use]
  pub const fn new(routee_paths: Vec<String>) -> Self {
    Self { routee_paths, allow_local_routees: true, use_roles: Vec::new() }
  }

  /// Overrides whether local routees are allowed.
  #[must_use]
  pub const fn with_allow_local_routees(mut self, allow: bool) -> Self {
    self.allow_local_routees = allow;
    self
  }

  /// Restricts routee selection to nodes that carry all of the given roles.
  #[must_use]
  pub fn with_use_roles(mut self, use_roles: Vec<String>) -> Self {
    self.use_roles = use_roles;
    self
  }

  /// Returns configured routee paths.
  #[must_use]
  pub fn routee_paths(&self) -> &[String] {
    &self.routee_paths
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
