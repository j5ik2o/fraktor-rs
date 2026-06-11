//! Provider-facing Split Brain Resolver hook.

#[cfg(test)]
#[path = "split_brain_resolver_provider_hook_test.rs"]
mod tests;

use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use super::{
  DowningDecision, DowningDecisionContext, DowningDecisionTrace, DowningInput, DowningProvider,
  DowningProviderCompatibility, DowningStrategyDecision, LeaseAcquisitionOutcome, LeaseMajorityPort,
  SplitBrainResolver, SplitBrainResolverConfig,
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
  /// Creates a hook and its provider compatibility metadata from configuration.
  #[must_use]
  pub fn new(config: SplitBrainResolverConfig) -> Self {
    let compatibility = Self::expected_compatibility(config);
    Self { compatibility, resolver: SplitBrainResolver::new(config) }
  }

  /// Creates a hook from externally supplied compatibility metadata.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the metadata does not match the supplied
  /// configuration.
  pub fn from_compatibility(
    config: SplitBrainResolverConfig,
    compatibility: DowningProviderCompatibility,
  ) -> Result<Self, ClusterProviderError> {
    if !Self::compatibility_matches(config, &compatibility) {
      return Err(ClusterProviderError::down(COMPATIBILITY_MISMATCH));
    }

    Ok(Self { compatibility, resolver: SplitBrainResolver::new(config) })
  }

  /// Returns compatibility metadata advertised by this hook.
  #[must_use]
  pub fn compatibility(&self) -> DowningProviderCompatibility {
    self.compatibility.clone()
  }

  /// Decides from a prebuilt context without a lease backend.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the evaluator trace reports a provider
  /// failure.
  pub fn decide_context(&mut self, context: &DowningDecisionContext) -> Result<DowningDecision, ClusterProviderError> {
    if context.explicit_down_authority().is_some() {
      return Ok(DowningDecision::Down);
    }
    let strategy_decision = self.resolver.decide(context);
    Self::map_trace(strategy_decision.trace())?;
    Ok(Self::provider_decision(context, &strategy_decision))
  }

  /// Decides from a prebuilt context with a lease backend port.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError::DownFailed`] when the evaluator trace reports a provider
  /// failure.
  pub fn decide_context_with_lease(
    &mut self,
    context: &DowningDecisionContext,
    lease_port: &mut dyn LeaseMajorityPort,
  ) -> Result<DowningDecision, ClusterProviderError> {
    if context.explicit_down_authority().is_some() {
      return Ok(DowningDecision::Down);
    }
    let strategy_decision = self.resolver.decide_with_lease(context, lease_port);
    Self::map_trace(strategy_decision.trace())?;
    Ok(Self::provider_decision(context, &strategy_decision))
  }

  fn expected_compatibility(config: SplitBrainResolverConfig) -> DowningProviderCompatibility {
    DowningProviderCompatibility::new(SPLIT_BRAIN_RESOLVER_PROVIDER_KEY).with_split_brain_resolver_config(config)
  }

  fn compatibility_matches(config: SplitBrainResolverConfig, compatibility: &DowningProviderCompatibility) -> bool {
    let expected = Self::expected_compatibility(config);
    compatibility.provider_key() == expected.provider_key()
      && compatibility.sbr_config_identity() == expected.sbr_config_identity()
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

  fn provider_decision(
    context: &DowningDecisionContext,
    strategy_decision: &DowningStrategyDecision,
  ) -> DowningDecision {
    if context
      .reachability_observer()
      .is_some_and(|observer| strategy_decision.downing_targets().iter().any(|target| target == observer))
    {
      return DowningDecision::Down;
    }
    strategy_decision.simple_decision()
  }
}

impl DowningProvider for SplitBrainResolverProviderHook {
  fn decide(&mut self, input: &DowningInput) -> Result<DowningDecision, ClusterProviderError> {
    let context = DowningDecisionContext::from_downing_input(input, Self::evaluation_time());
    SplitBrainResolverProviderHook::decide_context(self, &context)
  }

  fn decide_context(&mut self, context: &DowningDecisionContext) -> Result<DowningDecision, ClusterProviderError> {
    SplitBrainResolverProviderHook::decide_context(self, context)
  }
}
