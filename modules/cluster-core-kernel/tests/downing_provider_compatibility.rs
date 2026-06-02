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
  assert_eq!(settings.static_quorum_size(), None);
  assert_eq!(settings.with_static_quorum_size(3).static_quorum_size(), Some(3));
}

#[test]
fn downing_provider_compatibility_preserves_provider_key() {
  let compatibility = DowningProviderCompatibility::new("split-brain-resolver");

  assert_eq!(compatibility.provider_key(), "split-brain-resolver");
  assert!(compatibility.split_brain_resolver_settings().is_none());
  assert!(compatibility.sbr_settings_identity().is_none());
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
  assert_eq!(
    compatibility.sbr_settings_identity(),
    Some("stable-after-nanos=20000000000;active-strategy=keep-majority;down-all-when-unstable-nanos=15000000000")
  );
}

#[test]
#[should_panic(expected = "downing provider compatibility key must not be empty")]
fn downing_provider_compatibility_rejects_empty_provider_key() {
  let _compatibility = DowningProviderCompatibility::new("");
}

#[test]
fn split_brain_resolver_settings_identity_uses_strategy_identifier_and_durations() {
  let settings = SplitBrainResolverSettings::new(
    Duration::from_millis(2500),
    SplitBrainResolverStrategy::StaticQuorum,
    Duration::from_millis(750),
  )
  .with_static_quorum_size(3);

  let compatibility =
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(settings);

  assert_eq!(
    compatibility.sbr_settings_identity(),
    Some(
      "stable-after-nanos=2500000000;active-strategy=static-quorum;down-all-when-unstable-nanos=750000000;static-quorum-size=3"
    )
  );
}
