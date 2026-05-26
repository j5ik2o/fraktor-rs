//! No-op downing strategy implementation.

#[cfg(test)]
#[path = "noop_downing_provider_test.rs"]
mod tests;

use super::{DowningDecision, DowningInput, DowningProvider, FailureObservationKind};
use crate::ClusterProviderError;

/// Downing strategy that accepts all explicit down commands without side effects.
#[derive(Default)]
pub struct NoopDowningProvider;

impl NoopDowningProvider {
  /// Creates a new no-op downing strategy.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl DowningProvider for NoopDowningProvider {
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    match input {
      | DowningInput::ExplicitDown { .. } => Ok(DowningDecision::Down),
      | DowningInput::FailureObservation(observation) => match observation.kind() {
        | FailureObservationKind::Suspect | FailureObservationKind::Unreachable => Ok(DowningDecision::Defer),
        | FailureObservationKind::Recovered => Ok(DowningDecision::Keep),
      },
    }
  }
}
