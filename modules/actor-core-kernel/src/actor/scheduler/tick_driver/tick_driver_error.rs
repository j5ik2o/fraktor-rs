//! Tick driver error types.

use core::fmt::{Display, Formatter, Result as FmtResult};

#[cfg(test)]
#[path = "tick_driver_error_test.rs"]
mod tests;

/// Errors that can occur during tick driver operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickDriverError {
  /// Failed to spawn background task.
  SpawnFailed,
  /// Runtime handle not available.
  HandleUnavailable,
  /// Unsupported environment for auto-detection.
  UnsupportedEnvironment,
  /// Tick drift exceeded threshold.
  DriftExceeded,
  /// Driver has stopped unexpectedly.
  DriverStopped,
  /// Runtime flavor is not supported by this driver.
  UnsupportedExecutor,
  /// Resolution is zero or too small for safe operation.
  InvalidResolution,
}

impl Display for TickDriverError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::SpawnFailed => write!(f, "failed to spawn tick driver background task"),
      | Self::HandleUnavailable => write!(f, "runtime handle not available"),
      | Self::UnsupportedEnvironment => write!(f, "unsupported environment for tick driver auto-detection"),
      | Self::DriftExceeded => write!(f, "tick drift exceeded allowed threshold"),
      | Self::DriverStopped => write!(f, "tick driver has stopped unexpectedly"),
      | Self::UnsupportedExecutor => write!(f, "runtime flavor is not supported by this tick driver"),
      | Self::InvalidResolution => write!(f, "tick driver resolution is zero or too small for safe operation"),
    }
  }
}
