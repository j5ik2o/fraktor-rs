use core::time::Duration;

use super::*;
use crate::{
  ClusterTopology, ConfigValidation, JoinConfigCompatChecker,
  downing_provider::{DowningProviderCompatibility, SplitBrainResolverSettings, SplitBrainResolverStrategy},
  pub_sub::PubSubConfig,
};

#[test]
fn metrics_flag_and_address_are_preserved() {
  // metrics を有効に設定した構成がそのまま保持されることを確認
  let config = ClusterExtensionConfig::new().with_advertised_address("proto://node-a").with_metrics_enabled(true);
  assert_eq!(config.advertised_address(), "proto://node-a");
  assert!(config.metrics_enabled());

  // 無効設定に切り替えても正しく反映されることを確認
  let disabled = config.clone().with_metrics_enabled(false);
  assert!(!disabled.metrics_enabled());
  assert_eq!(disabled.advertised_address(), "proto://node-a");
}

#[test]
fn pubsub_config_is_preserved() {
  let custom = PubSubConfig::new(Duration::from_secs(5), Duration::from_secs(12));
  let config = ClusterExtensionConfig::new().with_pubsub_config(custom);
  assert_eq!(config.pubsub_config(), &custom);
}

#[test]
fn static_topology_is_preserved() {
  let topology =
    ClusterTopology::new(7, vec!["node-a".to_string()], vec!["node-b".to_string()], vec!["node-c".to_string()]);
  let config = ClusterExtensionConfig::new().with_static_topology(topology.clone());
  assert_eq!(config.static_topology(), Some(&topology));
}

#[test]
fn roles_are_sorted_and_deduplicated() {
  let config = ClusterExtensionConfig::new().with_roles(vec![
    "frontend".to_string(),
    "backend".to_string(),
    "frontend".to_string(),
  ]);
  assert_eq!(config.roles(), &["backend".to_string(), "frontend".to_string()]);
}

#[test]
fn app_version_is_preserved() {
  let config = ClusterExtensionConfig::new().with_app_version("2.3.4");
  assert_eq!(config.app_version(), "2.3.4");
}

#[test]
fn downing_provider_compatibility_is_preserved() {
  let compatibility = DowningProviderCompatibility::new("split-brain-resolver");

  let config = ClusterExtensionConfig::new().with_downing_provider_compatibility(compatibility.clone());

  assert_eq!(config.downing_provider_compatibility(), &compatibility);
}

#[test]
fn join_compatibility_key_manifest_lists_only_required_non_sensitive_keys() {
  let required_keys = ClusterExtensionConfig::required_join_compatibility_keys();

  assert_eq!(required_keys, &[
    "fraktor.cluster.pubsub.subscriber-timeout",
    "fraktor.cluster.pubsub.suspended-ttl",
    "fraktor.cluster.downing-provider.provider-key",
    "fraktor.cluster.downing-provider.split-brain-resolver.stable-after",
    "fraktor.cluster.downing-provider.split-brain-resolver.active-strategy",
    "fraktor.cluster.downing-provider.split-brain-resolver.down-all-when-unstable",
  ]);
  assert!(ClusterExtensionConfig::is_required_join_compatibility_key("fraktor.cluster.downing-provider.provider-key"));
  assert!(!ClusterExtensionConfig::is_required_join_compatibility_key("fraktor.cluster.advertised-address"));
  assert!(ClusterExtensionConfig::sensitive_join_compatibility_keys().is_empty());
  assert!(!ClusterExtensionConfig::is_sensitive_join_compatibility_key(
    "fraktor.cluster.downing-provider.provider-key"
  ));
}

#[test]
fn join_compatibility_reports_pubsub_mismatch() {
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(30)))
    .with_roles(vec!["backend".to_string()]);
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(5), Duration::from_secs(30)))
    .with_roles(vec!["frontend".to_string()]);

  let validation = local.check_join_compatibility(&joining);
  assert_eq!(validation, ConfigValidation::Incompatible { reason: "pubsub configuration mismatch".to_string() });
}

#[test]
fn join_compatibility_reports_downing_provider_mismatch() {
  let local =
    ClusterExtensionConfig::new().with_downing_provider_compatibility(DowningProviderCompatibility::new("sbr"));
  let joining =
    ClusterExtensionConfig::new().with_downing_provider_compatibility(DowningProviderCompatibility::new("noop"));

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "downing provider compatibility key mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_sbr_settings_mismatch_when_both_sides_configure_sbr() {
  let local_sbr = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let joining_sbr =
    SplitBrainResolverSettings::new(Duration::from_secs(20), SplitBrainResolverStrategy::KeepOldest, Duration::ZERO);
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(local_sbr),
  );
  let joining = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(joining_sbr),
  );

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "split brain resolver settings mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_sbr_settings_mismatch_against_missing_sbr_settings() {
  let sbr = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepOldest,
    Duration::from_secs(15),
  );
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(sbr),
  );
  let joining = ClusterExtensionConfig::new()
    .with_downing_provider_compatibility(DowningProviderCompatibility::new("split-brain-resolver"));

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "split brain resolver settings mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_sbr_timing_mismatch_when_strategy_matches() {
  let local_sbr = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let joining_sbr = SplitBrainResolverSettings::new(
    Duration::from_secs(21),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(local_sbr),
  );
  let joining = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(joining_sbr),
  );

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "split brain resolver settings mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_accepts_same_sbr_settings() {
  let sbr = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(sbr),
  );
  let joining = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(sbr),
  );

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Compatible);
}

#[test]
fn join_compatibility_accepts_same_pubsub_config() {
  let shared = PubSubConfig::new(Duration::from_secs(4), Duration::from_secs(40));
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(shared)
    .with_app_version("1.0.0")
    .with_advertised_address("proto://node-a")
    .with_roles(vec!["backend".to_string()]);
  let topology =
    ClusterTopology::new(7, vec!["node-a".to_string()], vec!["node-b".to_string()], vec!["node-c".to_string()]);
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(shared)
    .with_app_version("2.0.0")
    .with_advertised_address("proto://node-b")
    .with_static_topology(topology)
    .with_roles(vec!["frontend".to_string()]);

  let validation = local.check_join_compatibility(&joining);
  assert_eq!(validation, ConfigValidation::Compatible);
  assert!(validation.is_compatible());
}
