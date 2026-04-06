//! Join configuration compatibility checker.

use crate::core::{ClusterExtensionConfig, ConfigValidation};

/// Checks whether a joining node configuration is compatible with local settings.
pub trait JoinConfigCompatChecker {
  /// Validates compatibility between local and joining configurations.
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation;
}
