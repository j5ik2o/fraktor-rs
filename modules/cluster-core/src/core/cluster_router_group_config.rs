//! Minimal config for cluster router group behavior.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

/// Config for group-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterGroupConfig {
  routee_paths:        Vec<String>,
  allow_local_routees: bool,
}

impl ClusterRouterGroupConfig {
  /// Creates config with explicit routee paths.
  #[must_use]
  pub const fn new(routee_paths: Vec<String>) -> Self {
    Self { routee_paths, allow_local_routees: true }
  }

  /// Overrides whether local routees are allowed.
  #[must_use]
  pub const fn with_allow_local_routees(mut self, allow: bool) -> Self {
    self.allow_local_routees = allow;
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
}
