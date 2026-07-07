//! Cross-DC failure detector configuration validation errors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "cross_dc_failure_detector_config_error_test.rs"]
mod tests;

/// Cross-DC failure detector configuration validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDcFailureDetectorConfigError {
  /// Heartbeat interval is zero.
  ZeroHeartbeatInterval,
  /// Expected response after interval is zero.
  ZeroExpectedResponseAfter,
}

impl fmt::Display for CrossDcFailureDetectorConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::ZeroHeartbeatInterval => f.write_str("cross-DC heartbeat interval must be greater than zero"),
      | Self::ZeroExpectedResponseAfter => f.write_str("cross-DC expected response after must be greater than zero"),
    }
  }
}

impl Error for CrossDcFailureDetectorConfigError {}
