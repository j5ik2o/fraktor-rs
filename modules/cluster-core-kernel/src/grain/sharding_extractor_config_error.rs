//! Errors raised while constructing standard sharding extractors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "sharding_extractor_config_error_test.rs"]
mod tests;

/// Construction-time validation errors shared by the standard extractors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardingExtractorConfigError {
  /// The number of shards is zero.
  ShardCountZero,
}

impl fmt::Display for ShardingExtractorConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::ShardCountZero => f.write_str("number of shards must be greater than zero"),
    }
  }
}

impl Error for ShardingExtractorConfigError {}
