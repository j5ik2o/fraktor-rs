//! Provider-facing Split Brain Resolver hook.

#[cfg(test)]
#[path = "split_brain_resolver_provider_hook_test.rs"]
mod tests;

use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use super::{
  DowningDecision, DowningDecisionContext, DowningDecisionTrace, DowningInput, DowningProvider,
  DowningProviderCompatibility, LeaseAcquisitionOutcome, SplitBrainResolver, SplitBrainResolverSettings,
};
use crate::ClusterProviderError;

const SPLIT_BRAIN_RESOLVER_PROVIDER_KEY: &str = "split-brain-resolver";
const COMPATIBILITY_MISMATCH: &str = "split-brain-resolver compatibility metadata mismatch";

/// Provider hook that delegates downing evaluation to Split Brain Resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitBrainResolverProviderHook {
  compatibility: DowningProviderCompatibility,
  resolver:      SplitBrainResolver,
}

impl SplitBrainResolverProviderHook {
  /// Creates a hook and its provider compatibility metadata from settings.
  #[must_use]
  pub fn new(settings: SplitBrainResolverSettings) -> Self {
    let compatibility = Self::expected_compatibility(settings);
    Self { compatibility, resolver: SplitBrainResolver::new(settings) }
  }

  /// Creates a hook from externally supplied compatibility metadata.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the metadata does not match the supplied
  /// settings.
  pub fn from_compatibility(
    settings: SplitBrainResolverSettings,
    compatibility: DowningProviderCompatibility,
  ) -> Result<Self, ClusterProviderError> {
    if compatibility != Self::expected_compatibility(settings) {
      return Err(ClusterProviderError::down(COMPATIBILITY_MISMATCH));
    }

    Ok(Self { compatibility, resolver: SplitBrainResolver::new(settings) })
  }

  /// Returns compatibility metadata advertised by this hook.
  #[must_use]
  pub fn compatibility(&self) -> DowningProviderCompatibility {
    self.compatibility.clone()
  }

  fn expected_compatibility(settings: SplitBrainResolverSettings) -> DowningProviderCompatibility {
    DowningProviderCompatibility::new(SPLIT_BRAIN_RESOLVER_PROVIDER_KEY).with_split_brain_resolver_settings(settings)
  }

  const fn evaluation_time() -> TimerInstant {
    TimerInstant::zero(Duration::from_millis(1))
  }

  fn map_trace(trace: &DowningDecisionTrace) -> Result<(), ClusterProviderError> {
    if trace.lease_outcome() == Some(LeaseAcquisitionOutcome::BackendMissing) {
      return Err(ClusterProviderError::down(trace.reason()));
    }
    Ok(())
  }
}

impl DowningProvider for SplitBrainResolverProviderHook {
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    let context = DowningDecisionContext::from_downing_input(input, Self::evaluation_time());
    let strategy_decision = self.resolver.decide(&context);
    Self::map_trace(strategy_decision.trace())?;

    if context.explicit_down_authority().is_some() {
      return Ok(DowningDecision::Down);
    }

    Ok(strategy_decision.simple_decision())
  }
}
