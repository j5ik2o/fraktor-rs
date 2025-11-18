//! Endpoint snapshot for observability.

use alloc::string::String;

use super::association_state::AssociationState;

/// Snapshot of authority state for observability.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointSnapshot {
  /// Authority identifier.
  pub authority:   String,
  /// Current association state.
  pub state:       AssociationState,
  /// Monotonic timestamp of the last state change.
  pub last_change: u64,
  /// Optional reason for the last state transition.
  pub last_reason: Option<String>,
  /// Number of deferred messages.
  pub deferred:    usize,
}

impl EndpointSnapshot {
  /// Returns authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns association state.
  #[must_use]
  pub fn state(&self) -> &AssociationState {
    &self.state
  }

  /// Returns monotonic timestamp of the last change.
  #[must_use]
  pub const fn last_change(&self) -> u64 {
    self.last_change
  }

  /// Returns last transition reason when available.
  #[must_use]
  pub fn last_reason(&self) -> Option<&str> {
    self.last_reason.as_deref()
  }

  /// Returns number of deferred messages.
  #[must_use]
  pub const fn deferred(&self) -> usize {
    self.deferred
  }
}
