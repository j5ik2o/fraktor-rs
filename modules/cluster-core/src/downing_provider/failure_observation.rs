//! Failure observation accepted by a downing strategy.

use alloc::string::String;

use fraktor_utils_core_rs::time::TimerInstant;

use super::FailureObservationKind;

/// Availability observation for a member authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureObservation {
  authority:   String,
  kind:        FailureObservationKind,
  observed_at: TimerInstant,
}

impl FailureObservation {
  /// Creates a failure observation.
  #[must_use]
  pub fn new(authority: &str, kind: FailureObservationKind, observed_at: TimerInstant) -> Self {
    Self { authority: String::from(authority), kind, observed_at }
  }

  /// Returns the observed authority.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the observation kind.
  #[must_use]
  pub const fn kind(&self) -> FailureObservationKind {
    self.kind
  }

  /// Returns the observation time.
  #[must_use]
  pub const fn observed_at(&self) -> TimerInstant {
    self.observed_at
  }
}
