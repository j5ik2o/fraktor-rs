use core::time::Duration;

use super::*;
use crate::{
  ClusterTopology, ConfigValidation, JoinConfigCompatChecker,
  downing_provider::{DowningProviderCompatibility, SplitBrainResolverConfig, SplitBrainResolverStrategy},
  failure_detector::{FailureDetectorConfig, FailureDetectorConfigError},
  pub_sub::PubSubConfig,
  singleton::{ClusterSingletonConfigError, ClusterSingletonManagerConfig, ClusterSingletonProxyConfig},
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
fn default_failure_detector_config_is_preserved() {
  let config = ClusterExtensionConfig::new();

  assert_eq!(config.failure_detector_config(), &FailureDetectorConfig::default());
}

#[test]
fn custom_failure_detector_config_is_preserved() {
  let failure_detector_config = FailureDetectorConfig::new()
    .with_phi_threshold(8.0)
    .with_max_sample_size(128)
    .with_min_standard_deviation(Duration::from_millis(500))
    .with_acceptable_heartbeat_pause(Duration::from_secs(2))
    .with_first_heartbeat_estimate(Duration::from_secs(1));

  let config = ClusterExtensionConfig::new().with_failure_detector_config(failure_detector_config);

  assert_eq!(config.failure_detector_config(), &failure_detector_config);
}

#[test]
fn validate_delegates_to_failure_detector_config() {
  let config =
    ClusterExtensionConfig::new().with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(0.0));

  assert_eq!(config.validate(), Err(FailureDetectorConfigError::InvalidPhiThreshold));
}

#[test]
fn join_compatibility_accepts_same_failure_detector_config() {
  let failure_detector_config = FailureDetectorConfig::new()
    .with_phi_threshold(8.0)
    .with_max_sample_size(128)
    .with_min_standard_deviation(Duration::from_millis(500))
    .with_acceptable_heartbeat_pause(Duration::from_secs(2))
    .with_first_heartbeat_estimate(Duration::from_secs(1));
  let local = ClusterExtensionConfig::new().with_failure_detector_config(failure_detector_config);
  let joining = ClusterExtensionConfig::new().with_failure_detector_config(failure_detector_config);

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Compatible);
}

#[test]
fn join_compatibility_reports_failure_detector_config_mismatch_with_different_fields() {
  let local = ClusterExtensionConfig::new()
    .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(8.0).with_max_sample_size(128));
  let joining = ClusterExtensionConfig::new()
    .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(10.0).with_max_sample_size(256));

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.failure-detector mismatch: phi_threshold, max_sample_size".to_string(),
  });
}

#[test]
fn join_compatibility_key_manifest_separates_required_and_conditional_non_sensitive_keys() {
  let required_keys = ClusterExtensionConfig::required_join_compatibility_keys();
  let conditional_keys = ClusterExtensionConfig::conditional_join_compatibility_keys();

  assert_eq!(required_keys, &[
    "fraktor.cluster.pubsub.subscriber-timeout",
    "fraktor.cluster.pubsub.suspended-ttl",
    "fraktor.cluster.downing-provider.provider-key",
    "cluster.failure-detector",
  ]);
  assert_eq!(conditional_keys, &[
    "fraktor.cluster.downing-provider.split-brain-resolver.stable-after",
    "fraktor.cluster.downing-provider.split-brain-resolver.active-strategy",
    "fraktor.cluster.downing-provider.split-brain-resolver.down-all-when-unstable",
  ]);
  assert!(ClusterExtensionConfig::is_required_join_compatibility_key("fraktor.cluster.downing-provider.provider-key"));
  assert!(ClusterExtensionConfig::is_required_join_compatibility_key("cluster.failure-detector"));
  assert!(!ClusterExtensionConfig::is_required_join_compatibility_key("cluster.failure-detector.choice"));
  assert!(!ClusterExtensionConfig::is_required_join_compatibility_key("cluster.failure-detector.phi-threshold"));
  assert!(!ClusterExtensionConfig::is_required_join_compatibility_key(
    "fraktor.cluster.downing-provider.split-brain-resolver.stable-after"
  ));
  assert!(ClusterExtensionConfig::is_conditional_join_compatibility_key(
    "fraktor.cluster.downing-provider.split-brain-resolver.stable-after"
  ));
  assert!(!ClusterExtensionConfig::is_conditional_join_compatibility_key(
    "fraktor.cluster.downing-provider.provider-key"
  ));
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
  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.pubsub mismatch: pubsub configuration mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_each_required_key_mismatch() {
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(30)))
    .with_downing_provider_compatibility(DowningProviderCompatibility::new("sbr"));
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(5), Duration::from_secs(30)))
    .with_downing_provider_compatibility(DowningProviderCompatibility::new("noop"));

  let validation = local.check_join_compatibility(&joining);

  let ConfigValidation::Incompatible { reason } = validation else {
    panic!("required key mismatches should reject join");
  };
  assert!(reason.contains("cluster.pubsub mismatch: pubsub configuration mismatch"));
  assert!(reason.contains("cluster.downing-provider mismatch: downing provider compatibility key mismatch"));
}

#[test]
fn join_compatibility_reports_downing_provider_mismatch() {
  let local =
    ClusterExtensionConfig::new().with_downing_provider_compatibility(DowningProviderCompatibility::new("sbr"));
  let joining =
    ClusterExtensionConfig::new().with_downing_provider_compatibility(DowningProviderCompatibility::new("noop"));

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.downing-provider mismatch: downing provider compatibility key mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_sbr_settings_mismatch_when_both_sides_configure_sbr() {
  let local_sbr = SplitBrainResolverConfig::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let joining_sbr =
    SplitBrainResolverConfig::new(Duration::from_secs(20), SplitBrainResolverStrategy::KeepOldest, Duration::ZERO);
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(local_sbr),
  );
  let joining = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(joining_sbr),
  );

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.split-brain-resolver.config mismatch: split brain resolver config mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_reports_sbr_settings_mismatch_against_missing_sbr_settings() {
  let sbr = SplitBrainResolverConfig::new(
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
    reason: "cluster.split-brain-resolver.config mismatch: split brain resolver config mismatch".to_string(),
  });
}

#[test]
fn sbr_settings_checker_ignores_non_sbr_provider_pairs() {
  let sbr = SplitBrainResolverConfig::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepOldest,
    Duration::from_secs(15),
  );
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(sbr),
  );
  let joining =
    ClusterExtensionConfig::new().with_downing_provider_compatibility(DowningProviderCompatibility::new("noop"));

  assert!(split_brain_resolver_settings_are_compatible(&local, &joining));
}

#[test]
fn join_compatibility_reports_sbr_timing_mismatch_when_strategy_matches() {
  let local_sbr = SplitBrainResolverConfig::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let joining_sbr = SplitBrainResolverConfig::new(
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
    reason: "cluster.split-brain-resolver.config mismatch: split brain resolver config mismatch".to_string(),
  });
}

#[test]
fn join_compatibility_accepts_same_sbr_settings() {
  let sbr = SplitBrainResolverConfig::new(
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
fn join_compatibility_ignores_static_quorum_size_for_non_static_quorum_strategy() {
  let local_sbr = SplitBrainResolverConfig::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(15),
  );
  let joining_sbr = local_sbr.with_static_quorum_size(3);
  let local = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(local_sbr),
  );
  let joining = ClusterExtensionConfig::new().with_downing_provider_compatibility(
    DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(joining_sbr),
  );

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Compatible);
}

#[test]
fn join_compatibility_accepts_same_pubsub_config() {
  let shared = PubSubConfig::new(Duration::from_secs(4), Duration::from_secs(40));
  let local_topology =
    ClusterTopology::new(1, vec!["node-a".to_string()], vec!["node-b".to_string()], vec!["node-c".to_string()]);
  let joining_topology =
    ClusterTopology::new(2, vec!["node-d".to_string()], vec!["node-e".to_string()], vec!["node-f".to_string()]);
  let local = ClusterExtensionConfig::new()
    .with_advertised_address("proto://node-a")
    .with_metrics_enabled(true)
    .with_static_topology(local_topology)
    .with_pubsub_config(shared)
    .with_app_version("1.0.0")
    .with_advertised_address("proto://node-a")
    .with_roles(vec!["backend".to_string()]);
  let topology =
    ClusterTopology::new(7, vec!["node-a".to_string()], vec!["node-b".to_string()], vec!["node-c".to_string()]);
  let joining = ClusterExtensionConfig::new()
    .with_advertised_address("proto://node-b")
    .with_metrics_enabled(false)
    .with_static_topology(joining_topology)
    .with_pubsub_config(shared)
    .with_app_version("2.0.0")
    .with_advertised_address("proto://node-b")
    .with_static_topology(topology)
    .with_roles(vec!["frontend".to_string()]);

  let validation = local.check_join_compatibility(&joining);
  assert_eq!(validation, ConfigValidation::Compatible);
  assert!(validation.is_compatible());
}

// --- singleton 設定フィールドの保持と既定値 ---

#[test]
fn singleton_manager_config_default_is_preserved() {
  // ClusterExtensionConfig::new() が既定の ClusterSingletonManagerConfig を内包することを確認
  let config = ClusterExtensionConfig::new();
  assert_eq!(config.singleton_manager_config(), &ClusterSingletonManagerConfig::default());
}

#[test]
fn singleton_proxy_config_default_is_preserved() {
  // ClusterExtensionConfig::new() が既定の ClusterSingletonProxyConfig を内包することを確認
  let config = ClusterExtensionConfig::new();
  assert_eq!(config.singleton_proxy_config(), &ClusterSingletonProxyConfig::default());
}

#[test]
fn singleton_manager_config_is_preserved_via_setter() {
  // with_singleton_manager_config で設定した値が getter で正しく返されることを確認
  let custom = ClusterSingletonManagerConfig::new().with_singleton_name("my-singleton").with_min_hand_over_retries(20);
  let config = ClusterExtensionConfig::new().with_singleton_manager_config(custom.clone());
  assert_eq!(config.singleton_manager_config(), &custom);
}

#[test]
fn singleton_proxy_config_is_preserved_via_setter() {
  // with_singleton_proxy_config で設定した値が getter で正しく返されることを確認
  let custom = ClusterSingletonProxyConfig::new().with_singleton_name("my-singleton").with_buffer_size(500);
  let config = ClusterExtensionConfig::new().with_singleton_proxy_config(custom.clone());
  assert_eq!(config.singleton_proxy_config(), &custom);
}

// --- validate_singleton の委譲検証 ---

#[test]
fn validate_singleton_passes_with_default_settings() {
  // 既定値の singleton 設定は validate_singleton を通過する（要件 6.2）
  let config = ClusterExtensionConfig::new();
  assert_eq!(config.validate_singleton(), Ok(()));
}

#[test]
fn validate_singleton_delegates_to_manager_and_returns_error_on_empty_singleton_name() {
  // manager 設定が不正（空名）の場合、validate_singleton がエラーを返すことを確認
  let bad_manager = ClusterSingletonManagerConfig::new().with_singleton_name("");
  let config = ClusterExtensionConfig::new().with_singleton_manager_config(bad_manager);
  assert_eq!(config.validate_singleton(), Err(ClusterSingletonConfigError::EmptySingletonName));
}

#[test]
fn validate_singleton_delegates_to_proxy_and_returns_error_on_buffer_size_out_of_range() {
  // proxy 設定が不正（buffer_size 超過）の場合、validate_singleton がエラーを返すことを確認
  let bad_proxy = ClusterSingletonProxyConfig::new().with_buffer_size(10001);
  let config = ClusterExtensionConfig::new().with_singleton_proxy_config(bad_proxy);
  assert_eq!(config.validate_singleton(), Err(ClusterSingletonConfigError::BufferSizeOutOfRange { value: 10001 }));
}

#[test]
fn validate_does_not_change_signature_with_singleton_fields_added() {
  // 既存 validate() のシグネチャが変わっていないことを確認（要件 8.1 / 8.3）
  let config = ClusterExtensionConfig::new();
  let result: Result<(), FailureDetectorConfigError> = config.validate();
  assert_eq!(result, Ok(()));
}

// --- singleton 互換チェックの mismatch_detail ---

#[test]
fn join_compatibility_accepts_same_singleton_settings() {
  // singleton 設定が一致する場合、互換性チェックが Compatible を返すことを確認（要件 5.3）
  let settings = ClusterSingletonManagerConfig::new().with_singleton_name("my-singleton");
  let local = ClusterExtensionConfig::new().with_singleton_manager_config(settings.clone());
  let joining = ClusterExtensionConfig::new().with_singleton_manager_config(settings);

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Compatible);
}

#[test]
fn join_compatibility_reports_singleton_manager_mismatch_with_prefixed_field_names() {
  // manager 設定に差異がある場合、"manager."
  // プレフィックス付きフィールド名が理由に含まれることを確認（要件 5.2）
  let local_manager = ClusterSingletonManagerConfig::new().with_singleton_name("singleton-a");
  let joining_manager = ClusterSingletonManagerConfig::new().with_singleton_name("singleton-b");
  let local = ClusterExtensionConfig::new().with_singleton_manager_config(local_manager);
  let joining = ClusterExtensionConfig::new().with_singleton_manager_config(joining_manager);

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.singleton mismatch: manager.singleton_name".to_string(),
  });
}

#[test]
fn join_compatibility_reports_singleton_proxy_mismatch_with_prefixed_field_names() {
  // proxy 設定に差異がある場合、"proxy."
  // プレフィックス付きフィールド名が理由に含まれることを確認（要件 5.2）
  let local_proxy = ClusterSingletonProxyConfig::new().with_buffer_size(500);
  let joining_proxy = ClusterSingletonProxyConfig::new().with_buffer_size(1000);
  let local = ClusterExtensionConfig::new().with_singleton_proxy_config(local_proxy);
  let joining = ClusterExtensionConfig::new().with_singleton_proxy_config(joining_proxy);

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.singleton mismatch: proxy.buffer_size".to_string(),
  });
}

#[test]
fn join_compatibility_reports_both_manager_and_proxy_mismatch_fields() {
  // manager と proxy の両方に差異がある場合、両方の差異フィールドが結合して理由に含まれることを確認
  let local_manager = ClusterSingletonManagerConfig::new().with_singleton_name("singleton-a");
  let joining_manager = ClusterSingletonManagerConfig::new().with_singleton_name("singleton-b");
  let local_proxy = ClusterSingletonProxyConfig::new().with_buffer_size(500);
  let joining_proxy = ClusterSingletonProxyConfig::new().with_buffer_size(1000);
  let local =
    ClusterExtensionConfig::new().with_singleton_manager_config(local_manager).with_singleton_proxy_config(local_proxy);
  let joining = ClusterExtensionConfig::new()
    .with_singleton_manager_config(joining_manager)
    .with_singleton_proxy_config(joining_proxy);

  let validation = local.check_join_compatibility(&joining);

  assert_eq!(validation, ConfigValidation::Incompatible {
    reason: "cluster.singleton mismatch: manager.singleton_name, proxy.buffer_size".to_string(),
  });
}
