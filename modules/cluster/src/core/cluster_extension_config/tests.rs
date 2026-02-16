use super::*;

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
