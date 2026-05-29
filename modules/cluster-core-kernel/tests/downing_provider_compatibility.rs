use core::time::Duration;

use fraktor_cluster_core_kernel_rs::downing_provider::{
  DowningProviderCompatibility, SplitBrainResolverSettings, SplitBrainResolverStrategy,
};

#[test]
fn split_brain_resolver_strategy_returns_pekko_identifiers() {
  let strategies = [
    (SplitBrainResolverStrategy::KeepMajority, "keep-majority"),
    (SplitBrainResolverStrategy::LeaseMajority, "lease-majority"),
    (SplitBrainResolverStrategy::StaticQuorum, "static-quorum"),
    (SplitBrainResolverStrategy::KeepOldest, "keep-oldest"),
    (SplitBrainResolverStrategy::DownAll, "down-all"),
  ];

  let identifiers = strategies.map(|(strategy, _expected)| strategy.as_str());

  assert_eq!(identifiers, ["keep-majority", "lease-majority", "static-quorum", "keep-oldest", "down-all"]);
}

#[test]
fn split_brain_resolver_settings_preserve_active_strategy_and_durations() {
  let settings = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );

  assert_eq!(settings.stable_after(), Duration::from_secs(20));
  assert_eq!(settings.active_strategy(), SplitBrainResolverStrategy::KeepMajority);
  assert_eq!(settings.down_all_when_unstable(), Duration::from_secs(15));
}

#[test]
fn downing_provider_compatibility_preserves_provider_key() {
  let compatibility = DowningProviderCompatibility::new("split-brain-resolver");

  assert_eq!(compatibility.provider_key(), "split-brain-resolver");
  assert!(compatibility.split_brain_resolver_settings().is_none());
}

#[test]
fn downing_provider_compatibility_preserves_split_brain_resolver_settings() {
  let settings = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );

  let compatibility =
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(settings);

  assert_eq!(compatibility.provider_key(), "split-brain-resolver");
  assert_eq!(compatibility.split_brain_resolver_settings(), Some(&settings));
}

#[test]
#[should_panic(expected = "downing provider compatibility key must not be empty")]
fn downing_provider_compatibility_rejects_empty_provider_key() {
  let _compatibility = DowningProviderCompatibility::new("");
}
