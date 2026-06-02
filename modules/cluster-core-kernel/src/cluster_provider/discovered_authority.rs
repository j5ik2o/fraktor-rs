//! Provider-neutral discovered authority value.

use alloc::string::String;

use fraktor_utils_core_rs::time::TimerInstant;

#[cfg(test)]
#[path = "discovered_authority_test.rs"]
mod tests;

/// Authority observed from a discovery backend without backend-specific metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAuthority {
  authority:       String,
  source_identity: String,
  observed_at:     TimerInstant,
}

impl DiscoveredAuthority {
  /// Creates a discovered authority value.
  #[must_use]
  pub const fn new(authority: String, source_identity: String, observed_at: TimerInstant) -> Self {
    Self { authority, source_identity, observed_at }
  }

  /// Returns the discovered authority.
  #[must_use]
  pub const fn authority(&self) -> &str {
    self.authority.as_str()
  }

  /// Returns the discovery source identity used for observability.
  #[must_use]
  pub const fn source_identity(&self) -> &str {
    self.source_identity.as_str()
  }

  /// Returns when the authority was observed.
  #[must_use]
  pub const fn observed_at(&self) -> TimerInstant {
    self.observed_at
  }

  /// Returns only the authority value used by placement and membership inputs.
  #[must_use]
  pub fn to_authority(&self) -> String {
    self.authority.clone()
  }
}
