//! Stable compatibility key identity.

#[cfg(test)]
#[path = "cluster_compatibility_key_test.rs"]
mod tests;

/// Stable key name used by cluster join compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClusterCompatibilityKey {
  name:             &'static str,
  exclusion_reason: Option<&'static str>,
}

impl ClusterCompatibilityKey {
  pub(crate) const fn required(name: &'static str) -> Self {
    Self { name, exclusion_reason: None }
  }

  pub(crate) const fn excluded(name: &'static str, exclusion_reason: &'static str) -> Self {
    Self { name, exclusion_reason: Some(exclusion_reason) }
  }

  /// Returns the stable key name.
  #[must_use]
  pub const fn name(&self) -> &'static str {
    self.name
  }

  /// Returns why this key is excluded from join compatibility comparison.
  #[must_use]
  pub const fn exclusion_reason(&self) -> Option<&'static str> {
    self.exclusion_reason
  }
}
