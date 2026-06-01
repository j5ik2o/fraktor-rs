//! Immutable key set for cluster compatibility catalog slices.

#[cfg(test)]
#[path = "cluster_compatibility_key_set_test.rs"]
mod tests;

use crate::topology::{ClusterCompatibilityKey, ClusterCompatibilityKeyCatalog};

/// Immutable required and excluded key set for cluster compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterCompatibilityKeySet {
  required_keys: &'static [ClusterCompatibilityKey],
  excluded_keys: &'static [ClusterCompatibilityKey],
}

impl ClusterCompatibilityKeySet {
  /// Returns the baseline cluster compatibility key set.
  #[must_use]
  pub const fn cluster_compatibility() -> Self {
    Self {
      required_keys: ClusterCompatibilityKeyCatalog::required_keys(),
      excluded_keys: ClusterCompatibilityKeyCatalog::excluded_keys(),
    }
  }

  /// Returns required keys compared by join compatibility.
  #[must_use]
  pub const fn required_keys(&self) -> &'static [ClusterCompatibilityKey] {
    self.required_keys
  }

  /// Returns keys excluded from join compatibility comparison.
  #[must_use]
  pub const fn excluded_keys(&self) -> &'static [ClusterCompatibilityKey] {
    self.excluded_keys
  }
}
