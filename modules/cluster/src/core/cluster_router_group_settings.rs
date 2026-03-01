//! Minimal settings for cluster router group behavior.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

/// Settings for group-style cluster routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRouterGroupSettings {
  routee_paths:        Vec<String>,
  allow_local_routees: bool,
}

impl ClusterRouterGroupSettings {
  /// Creates settings with explicit routee paths.
  ///
  /// # Panics
  ///
  /// Panics when `routee_paths` is empty.
  #[must_use]
  pub fn new(routee_paths: Vec<String>) -> Self {
    assert!(!routee_paths.is_empty(), "routee paths must not be empty");
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
