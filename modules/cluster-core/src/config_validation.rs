//! Join configuration validation result.

use alloc::string::String;

/// Outcome of join configuration compatibility checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValidation {
  /// Configurations are compatible and joining is allowed.
  Compatible,
  /// Configurations are incompatible and joining must be rejected.
  Incompatible {
    /// Human-readable reason for incompatibility.
    reason: String,
  },
}

impl ConfigValidation {
  /// Returns true if the configuration is compatible.
  #[must_use]
  pub const fn is_compatible(&self) -> bool {
    matches!(self, Self::Compatible)
  }
}
