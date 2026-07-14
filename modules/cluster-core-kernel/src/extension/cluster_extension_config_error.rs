//! Cluster extension configuration validation errors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

use crate::failure_detector::FailureDetectorConfigError;

/// Errors returned when cluster extension configuration is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterExtensionConfigError {
  /// Failure detector configuration is invalid.
  FailureDetector(FailureDetectorConfigError),
  /// Grain idle passivation cannot be represented by the runtime's second-based clock.
  GrainIdlePassivationThresholdBelowOneSecond,
  /// Grain idle passivation contains a fractional second unsupported by the runtime clock.
  GrainIdlePassivationThresholdNotWholeSeconds,
}

impl fmt::Display for ClusterExtensionConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::FailureDetector(error) => error.fmt(f),
      | Self::GrainIdlePassivationThresholdBelowOneSecond => {
        f.write_str("grain idle passivation threshold must be at least one second")
      },
      | Self::GrainIdlePassivationThresholdNotWholeSeconds => {
        f.write_str("grain idle passivation threshold must use whole seconds")
      },
    }
  }
}

impl Error for ClusterExtensionConfigError {}

impl From<FailureDetectorConfigError> for ClusterExtensionConfigError {
  fn from(value: FailureDetectorConfigError) -> Self {
    Self::FailureDetector(value)
  }
}
