//! Cluster extension configuration validation errors.

use core::{
  error::Error,
  fmt::{Display, Formatter, Result as FmtResult},
};

use super::ClusterShardingSettingsError;
use crate::failure_detector::{CrossDcFailureDetectorConfigError, FailureDetectorConfigError};

#[cfg(test)]
#[path = "cluster_extension_config_validation_error_test.rs"]
mod tests;

/// Validation errors produced by [`ClusterExtensionConfig`](super::ClusterExtensionConfig).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterExtensionConfigValidationError {
  /// Failure detector configuration is invalid.
  FailureDetector(FailureDetectorConfigError),
  /// Cross-DC failure detector configuration is invalid.
  CrossDcFailureDetector(CrossDcFailureDetectorConfigError),
  /// Cluster sharding settings are invalid.
  ShardingSettings(ClusterShardingSettingsError),
}

impl Display for ClusterExtensionConfigValidationError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::FailureDetector(error) => write!(f, "failure detector config: {error}"),
      | Self::CrossDcFailureDetector(error) => write!(f, "cross-DC failure detector config: {error}"),
      | Self::ShardingSettings(error) => write!(f, "sharding settings: {error}"),
    }
  }
}

impl Error for ClusterExtensionConfigValidationError {}
