//! Provider-neutral discovery outcome value.

use alloc::{string::String, vec::Vec};

use fraktor_utils_core_rs::time::TimerInstant;

use super::DiscoveredAuthority;
use crate::ClusterProviderError;

#[cfg(test)]
#[path = "discovery_result_test.rs"]
mod tests;

/// Discovery backend outcome normalized at the cluster provider boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryResult {
  /// Authorities were discovered successfully.
  Discovered(Vec<DiscoveredAuthority>),
  /// Discovery completed successfully without authorities.
  Empty(String, TimerInstant),
  /// Discovery failed without producing destructive topology input.
  Failed(String, TimerInstant, ClusterProviderError),
}

impl DiscoveryResult {
  /// Creates a successful discovery result.
  #[must_use]
  pub const fn discovered(authorities: Vec<DiscoveredAuthority>) -> Self {
    Self::Discovered(authorities)
  }

  /// Creates an empty successful discovery result.
  #[must_use]
  pub const fn empty(source_identity: String, observed_at: TimerInstant) -> Self {
    Self::Empty(source_identity, observed_at)
  }

  /// Creates a failed discovery result.
  #[must_use]
  pub const fn failed(source_identity: String, observed_at: TimerInstant, error: ClusterProviderError) -> Self {
    Self::Failed(source_identity, observed_at, error)
  }

  /// Returns discovered authorities.
  #[must_use]
  pub const fn authorities(&self) -> &[DiscoveredAuthority] {
    match self {
      | Self::Discovered(authorities) => authorities.as_slice(),
      | Self::Empty(_, _) | Self::Failed(_, _, _) => &[],
    }
  }

  /// Returns whether discovery completed successfully without authorities.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    matches!(self, Self::Empty(_, _))
  }

  /// Returns whether discovery failed.
  #[must_use]
  pub const fn is_failed(&self) -> bool {
    matches!(self, Self::Failed(_, _, _))
  }

  /// Returns the discovery source identity for result-level outcomes.
  #[must_use]
  pub const fn source_identity(&self) -> Option<&str> {
    match self {
      | Self::Discovered(_) => None,
      | Self::Empty(source_identity, _) | Self::Failed(source_identity, _, _) => Some(source_identity.as_str()),
    }
  }

  /// Returns the result-level observation time.
  #[must_use]
  pub const fn observed_at(&self) -> Option<TimerInstant> {
    match self {
      | Self::Discovered(_) => None,
      | Self::Empty(_, observed_at) | Self::Failed(_, observed_at, _) => Some(*observed_at),
    }
  }

  /// Returns the observable failure, when discovery failed.
  #[must_use]
  pub const fn error(&self) -> Option<&ClusterProviderError> {
    match self {
      | Self::Failed(_, _, error) => Some(error),
      | Self::Discovered(_) | Self::Empty(_, _) => None,
    }
  }

  /// Returns only authority values suitable for placement and membership input.
  #[must_use]
  pub fn to_authorities(&self) -> Vec<String> {
    self.authorities().iter().map(DiscoveredAuthority::to_authority).collect()
  }
}
