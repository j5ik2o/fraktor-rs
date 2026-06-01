//! Data center domain primitive for cluster membership.

#[cfg(test)]
#[path = "data_center_test.rs"]
mod tests;

use alloc::string::String;

/// Identifies the data center a cluster member belongs to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataCenter {
  name: String,
}

impl DataCenter {
  /// Creates a data center from an explicit name.
  #[must_use]
  pub fn new(name: impl Into<String>) -> Self {
    Self { name: name.into() }
  }

  /// Returns the observable data center name.
  #[must_use]
  pub const fn as_str(&self) -> &str {
    self.name.as_str()
  }
}

impl Default for DataCenter {
  fn default() -> Self {
    Self::new("default")
  }
}
