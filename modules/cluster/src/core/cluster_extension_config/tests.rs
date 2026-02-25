use super::*;
use crate::core::{ConfigValidation, JoinConfigCompatChecker};

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
  let custom =
    crate::core::pub_sub::PubSubConfig::new(core::time::Duration::from_secs(5), core::time::Duration::from_secs(12));
  let config = ClusterExtensionConfig::new().with_pubsub_config(custom);
  assert_eq!(config.pubsub_config(), &custom);
}

#[test]
fn static_topology_is_preserved() {
  let topology = crate::core::cluster_topology::ClusterTopology::new(
    7,
    vec!["node-a".to_string()],
    vec!["node-b".to_string()],
    vec!["node-c".to_string()],
  );
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
fn join_compatibility_reports_pubsub_mismatch() {
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(crate::core::pub_sub::PubSubConfig::new(
      core::time::Duration::from_secs(3),
      core::time::Duration::from_secs(30),
    ))
    .with_roles(vec!["backend".to_string()]);
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(crate::core::pub_sub::PubSubConfig::new(
      core::time::Duration::from_secs(5),
      core::time::Duration::from_secs(30),
    ))
    .with_roles(vec!["frontend".to_string()]);

  let validation = local.check_join_compatibility(&joining);
  assert_eq!(validation, ConfigValidation::Incompatible { reason: "pubsub configuration mismatch".to_string() });
}

#[test]
fn join_compatibility_accepts_same_pubsub_config() {
  let shared =
    crate::core::pub_sub::PubSubConfig::new(core::time::Duration::from_secs(4), core::time::Duration::from_secs(40));
  let local = ClusterExtensionConfig::new().with_pubsub_config(shared).with_roles(vec!["backend".to_string()]);
  let joining = ClusterExtensionConfig::new().with_pubsub_config(shared).with_roles(vec!["frontend".to_string()]);

  let validation = local.check_join_compatibility(&joining);
  assert_eq!(validation, ConfigValidation::Compatible);
  assert!(validation.is_compatible());
}
