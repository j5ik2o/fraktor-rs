//! Singleton settings validation errors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "cluster_singleton_config_error_test.rs"]
mod tests;

/// Singleton configuration validation errors.
///
/// Each variant identifies the configuration item that caused the validation
/// failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterSingletonConfigError {
  /// Singleton name is an empty string.
  EmptySingletonName,
  /// Buffer size is outside the allowed range (0..=10000).
  BufferSizeOutOfRange {
    /// The out-of-range value that was supplied.
    value: u32,
  },
  /// Hand-over retry interval is zero or negative.
  NonPositiveHandOverRetryInterval,
  /// Singleton identification interval is zero or negative.
  NonPositiveIdentificationInterval,
  /// Lease implementation name is an empty string.
  EmptyLeaseImplementation,
  /// Lease retry interval is zero or negative.
  NonPositiveLeaseRetryInterval,
}

impl fmt::Display for ClusterSingletonConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::EmptySingletonName => f.write_str("singleton name must not be empty"),
      | Self::BufferSizeOutOfRange { value } => {
        write!(f, "buffer size {value} is out of the allowed range (0..=10000)")
      },
      | Self::NonPositiveHandOverRetryInterval => f.write_str("hand-over retry interval must be greater than zero"),
      | Self::NonPositiveIdentificationInterval => f.write_str("identification interval must be greater than zero"),
      | Self::EmptyLeaseImplementation => f.write_str("lease implementation name must not be empty"),
      | Self::NonPositiveLeaseRetryInterval => f.write_str("lease retry interval must be greater than zero"),
    }
  }
}

impl Error for ClusterSingletonConfigError {}
