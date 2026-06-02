use core::time::Duration;

use crate::{
  ClusterProviderError,
  downing_provider::{
    DowningDecision, DowningInput, DowningProvider, DowningProviderCompatibility, SplitBrainResolverProviderHook,
    SplitBrainResolverSettings, SplitBrainResolverStrategy,
  },
};

#[test]
fn provider_hook_exposes_sbr_compatibility_metadata() {
  let settings = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  );
  let hook = SplitBrainResolverProviderHook::new(settings);

  let compatibility = hook.compatibility();

  assert_eq!(compatibility.provider_key(), "split-brain-resolver");
  assert_eq!(compatibility.split_brain_resolver_settings(), Some(&settings));
  assert_eq!(
    compatibility.sbr_settings_identity(),
    Some("stable-after-nanos=20000000000;active-strategy=keep-majority;down-all-when-unstable-nanos=30000000000"),
  );
}

#[test]
fn provider_hook_rejects_mismatched_metadata() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepMajority, Duration::from_secs(30));
  let compatibility = DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepOldest, Duration::from_secs(30)),
  );

  let err = SplitBrainResolverProviderHook::from_compatibility(settings, compatibility).expect_err("metadata mismatch");

  assert!(matches!(err, ClusterProviderError::DownFailed(_)));
  assert!(err.reason().contains("split-brain-resolver compatibility metadata mismatch"));
}

#[test]
fn provider_hook_maps_explicit_down_without_membership_snapshot() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);

  let decision = hook.decide(&DowningInput::explicit_down("node-a:2552"));

  assert_eq!(decision, Ok(DowningDecision::Down));
}

#[test]
fn provider_hook_maps_decision_failure_to_cluster_provider_error() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);

  let err = hook.decide(&DowningInput::explicit_down("node-a:2552")).expect_err("missing lease backend");

  assert!(matches!(err, ClusterProviderError::DownFailed(_)));
  assert_eq!(err.reason(), "lease backend missing");
}
