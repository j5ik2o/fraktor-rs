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

  /// Restricts routee selection to members with any of the supplied roles.
  #[must_use]
  pub fn with_use_roles(mut self, roles: Vec<String>) -> Self {
    self.use_roles = normalize_roles(roles);
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

  /// Returns role constraints for routee selection.
  #[must_use]
  pub fn use_roles(&self) -> &[String] {
    &self.use_roles
  }
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
