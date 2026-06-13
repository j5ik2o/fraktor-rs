//! Grow-only boolean flag CRDT.

#[cfg(test)]
#[path = "flag_test.rs"]
mod tests;

use super::ReplicatedData;

/// Boolean CRDT that can only move from disabled to enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
  enabled: bool,
}

impl Flag {
  /// Returns the initial disabled flag.
  #[must_use]
  pub const fn disabled() -> Self {
    Self { enabled: false }
  }

  /// Returns true when this flag is enabled.
  #[must_use]
  pub const fn is_enabled(&self) -> bool {
    self.enabled
  }

  /// Returns an enabled flag.
  #[must_use]
  pub fn switch_on(&self) -> Self {
    self.merge(&Self { enabled: true })
  }
}

impl ReplicatedData for Flag {
  fn merge(&self, other: &Self) -> Self {
    Self { enabled: self.enabled || other.enabled }
  }
}

impl Default for Flag {
  fn default() -> Self {
    Self::disabled()
  }
}
