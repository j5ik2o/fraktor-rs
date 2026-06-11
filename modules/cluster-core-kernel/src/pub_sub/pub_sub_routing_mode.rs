//! Routing mode used by distributed pub-sub path delivery.

use alloc::format;

use super::PubSubError;

/// Path delivery routing mode for distributed pub-sub `Send`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PubSubRoutingMode {
  /// Selects one matching target randomly.
  #[default]
  Random,
  /// Selects matching targets in round-robin order.
  RoundRobin,
}

impl PubSubRoutingMode {
  /// Parses a routing mode from a configuration value.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidConfig`] when the value is not supported by distributed
  /// pub-sub.
  pub fn try_from_name(value: &str) -> Result<Self, PubSubError> {
    match value {
      | "random" => Ok(Self::Random),
      | "round-robin" => Ok(Self::RoundRobin),
      | other => Err(PubSubError::InvalidConfig { reason: format!("unsupported routing mode: {other}") }),
    }
  }
}
