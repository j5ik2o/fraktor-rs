//! Downing strategy abstractions for member down decisions.

mod downing_decision;
mod downing_input;
mod failure_observation;
mod failure_observation_kind;
mod noop_downing_provider;

pub use downing_decision::DowningDecision;
pub use downing_input::DowningInput;
pub use failure_observation::FailureObservation;
pub use failure_observation_kind::FailureObservationKind;
pub use noop_downing_provider::NoopDowningProvider;

use crate::ClusterProviderError;

/// Strategy hook invoked before a member is downed.
pub trait DowningProvider: Send + Sync {
  /// Decides how cluster core should handle the downing input.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when the strategy cannot decide.
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError>;
}
