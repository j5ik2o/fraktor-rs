//! Downing strategy abstractions for member down decisions.

mod downing_decision;
mod downing_decision_context;
mod downing_decision_trace;
mod downing_input;
mod downing_provider_compatibility;
mod downing_strategy_decision;
mod failure_observation;
mod failure_observation_kind;
mod lease_acquisition_outcome;
mod lease_majority_port;
mod noop_downing_provider;
mod split_brain_resolver;
mod split_brain_resolver_provider_hook;
mod split_brain_resolver_settings;
mod split_brain_resolver_strategy;

pub use downing_decision::DowningDecision;
pub use downing_decision_context::DowningDecisionContext;
pub use downing_decision_trace::DowningDecisionTrace;
pub use downing_input::DowningInput;
pub use downing_provider_compatibility::DowningProviderCompatibility;
pub use downing_strategy_decision::DowningStrategyDecision;
pub use failure_observation::FailureObservation;
pub use failure_observation_kind::FailureObservationKind;
pub use lease_acquisition_outcome::LeaseAcquisitionOutcome;
pub use lease_majority_port::LeaseMajorityPort;
pub use noop_downing_provider::NoopDowningProvider;
pub use split_brain_resolver::SplitBrainResolver;
pub use split_brain_resolver_provider_hook::SplitBrainResolverProviderHook;
pub use split_brain_resolver_settings::SplitBrainResolverSettings;
pub use split_brain_resolver_strategy::SplitBrainResolverStrategy;

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
