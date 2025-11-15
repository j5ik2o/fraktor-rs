//! Tick driver error types.

use core::fmt;

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
}

impl fmt::Display for TickDriverError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::SpawnFailed => write!(f, "failed to spawn tick driver background task"),
      | Self::HandleUnavailable => write!(f, "runtime handle not available"),
      | Self::UnsupportedEnvironment => write!(f, "unsupported environment for tick driver auto-detection"),
      | Self::DriftExceeded => write!(f, "tick drift exceeded allowed threshold"),
      | Self::DriverStopped => write!(f, "tick driver has stopped unexpectedly"),
    }
  }
}
