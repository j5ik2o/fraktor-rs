//! Lifecycle-aware seed node join input processing.

use alloc::{string::String, vec::Vec};

use super::SeedNodeInput;
use crate::ClusterProviderError;

#[cfg(test)]
#[path = "seed_node_process_test.rs"]
mod tests;

/// Converts lifecycle seed authorities into provider-neutral join input.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SeedNodeProcess {
  is_shutdown: bool,
}

impl SeedNodeProcess {
  /// Creates a seed node process.
  #[must_use]
  pub const fn new() -> Self {
    Self { is_shutdown: false }
  }

  /// Starts seed processing for member mode.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when a seed authority is invalid.
  pub fn start_member(&mut self, input: &SeedNodeInput) -> Result<Vec<String>, ClusterProviderError> {
    if self.is_shutdown {
      return Ok(Vec::new());
    }

    Self::validate_seed_authorities(input)?;

    let mut joins = Vec::new();
    for authority in input.seed_authorities() {
      if authority == input.advertised_authority() || joins.contains(authority) {
        continue;
      }
      joins.push(authority.clone());
    }
    Ok(joins)
  }

  /// Starts seed processing for client mode.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when a seed authority is invalid.
  pub fn start_client(&mut self, input: &SeedNodeInput) -> Result<Vec<String>, ClusterProviderError> {
    if self.is_shutdown {
      return Ok(Vec::new());
    }

    Self::validate_seed_authorities(input)?;
    Ok(Vec::new())
  }

  /// Shuts down the seed process.
  ///
  /// # Errors
  ///
  /// This implementation does not fail, but returns `Result` to match provider
  /// lifecycle contracts.
  pub const fn shutdown(&mut self) -> Result<(), ClusterProviderError> {
    self.is_shutdown = true;
    Ok(())
  }

  fn validate_seed_authorities(input: &SeedNodeInput) -> Result<(), ClusterProviderError> {
    if input.seed_authorities().iter().any(|authority| !Self::is_valid_authority(authority)) {
      return Err(ClusterProviderError::join("invalid seed authority"));
    }
    Ok(())
  }

  fn is_valid_authority(authority: &str) -> bool {
    !authority.is_empty() && !authority.chars().any(char::is_whitespace)
  }
}
