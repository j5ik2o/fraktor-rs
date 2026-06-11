use core::time::Duration;

use fraktor_cluster_core_kernel_rs::singleton::{
  ClusterSingletonManagerConfig, ClusterSingletonProxyConfig, LeaseUsageConfig,
};

use crate::ClusterSingletonConfig;

// ── 既定値テスト ──────────────────────────────────────────────────────────

#[test]
fn default_derives_match_kernel_manager_defaults() {
  // 統合設定の既定値から導出した manager 設定が kernel の既定値と一致する（要件 3.4）
  let unified = ClusterSingletonConfig::new();
  let derived_manager = unified.to_manager_config("singleton");
  let kernel_manager = ClusterSingletonManagerConfig::new();

  assert_eq!(derived_manager.role(), kernel_manager.role());
  assert_eq!(derived_manager.removal_margin(), kernel_manager.removal_margin());
  assert_eq!(derived_manager.hand_over_retry_interval(), kernel_manager.hand_over_retry_interval());
  assert_eq!(derived_manager.min_hand_over_retries(), kernel_manager.min_hand_over_retries());
  assert_eq!(derived_manager.lease_config(), kernel_manager.lease_config());
}

#[test]
fn default_derives_match_kernel_proxy_defaults() {
  // 統合設定の既定値から導出した proxy 設定が kernel の既定値と一致する（要件 3.4）
  let unified = ClusterSingletonConfig::new();
  let derived_proxy = unified.to_proxy_config("singleton");
  let kernel_proxy = ClusterSingletonProxyConfig::new();

  assert_eq!(derived_proxy.role(), kernel_proxy.role());
  assert_eq!(derived_proxy.data_center(), kernel_proxy.data_center());
  assert_eq!(derived_proxy.singleton_identification_interval(), kernel_proxy.singleton_identification_interval());
  assert_eq!(derived_proxy.buffer_size(), kernel_proxy.buffer_size());
}

// ── 導出無損失テスト ──────────────────────────────────────────────────────

#[test]
fn manager_derivation_is_lossless() {
  // 非既定値で構築した統合設定から manager を導出したとき、各項目の値が変化しない（要件 3.2）
  let lease = LeaseUsageConfig::new("my-lease", Duration::from_secs(2));
  let unified = ClusterSingletonConfig::new()
    .with_role("backend")
    .with_removal_margin(Duration::from_secs(5))
    .with_hand_over_retry_interval(Duration::from_millis(500))
    .with_min_hand_over_retries(20)
    .with_lease_config(lease.clone());

  let manager = unified.to_manager_config("my-singleton");

  assert_eq!(manager.singleton_name(), "my-singleton");
  assert_eq!(manager.role(), Some("backend"));
  assert_eq!(manager.removal_margin(), Some(Duration::from_secs(5)));
  assert_eq!(manager.hand_over_retry_interval(), Duration::from_millis(500));
  assert_eq!(manager.min_hand_over_retries(), 20);
  assert_eq!(manager.lease_config(), Some(&lease));
}

#[test]
fn proxy_derivation_is_lossless() {
  // 非既定値で構築した統合設定から proxy を導出したとき、各項目の値が変化しない（要件 3.3）
  use fraktor_cluster_core_kernel_rs::membership::DataCenter;

  let dc = DataCenter::new("dc-west");
  let unified = ClusterSingletonConfig::new()
    .with_role("frontend")
    .with_data_center(dc.clone())
    .with_singleton_identification_interval(Duration::from_millis(250))
    .with_buffer_size(500);

  let proxy = unified.to_proxy_config("my-proxy");

  assert_eq!(proxy.singleton_name(), "my-proxy");
  assert_eq!(proxy.role(), Some("frontend"));
  assert_eq!(proxy.data_center(), Some(&dc));
  assert_eq!(proxy.singleton_identification_interval(), Duration::from_millis(250));
  assert_eq!(proxy.buffer_size(), 500);
}

// ── manager 非対象項目が導出に影響しない ──────────────────────────────────

#[test]
fn proxy_only_fields_do_not_affect_manager_derivation() {
  // data_center / identification_interval / buffer_size は manager に存在しない（要件 3.2）
  use fraktor_cluster_core_kernel_rs::membership::DataCenter;

  let dc = DataCenter::new("dc-east");
  let unified = ClusterSingletonConfig::new()
    .with_data_center(dc)
    .with_singleton_identification_interval(Duration::from_millis(750))
    .with_buffer_size(2000);

  let manager = unified.to_manager_config("test");
  let default_manager = ClusterSingletonManagerConfig::new().with_singleton_name("test");

  // proxy 専用フィールドを変えても manager の全項目は変化しない
  assert_eq!(manager.role(), default_manager.role());
  assert_eq!(manager.removal_margin(), default_manager.removal_margin());
  assert_eq!(manager.hand_over_retry_interval(), default_manager.hand_over_retry_interval());
  assert_eq!(manager.min_hand_over_retries(), default_manager.min_hand_over_retries());
  assert_eq!(manager.lease_config(), default_manager.lease_config());
}

#[test]
fn manager_only_fields_do_not_affect_proxy_derivation() {
  // removal_margin / hand_over_retry_interval / min_hand_over_retries / lease_config は proxy
  // に存在しない（要件 3.3）
  let lease = LeaseUsageConfig::new("lease-impl", Duration::from_secs(3));
  let unified = ClusterSingletonConfig::new()
    .with_removal_margin(Duration::from_secs(10))
    .with_hand_over_retry_interval(Duration::from_millis(750))
    .with_min_hand_over_retries(25)
    .with_lease_config(lease);

  let proxy = unified.to_proxy_config("test");
  let default_proxy = ClusterSingletonProxyConfig::new().with_singleton_name("test");

  // manager 専用フィールドを変えても proxy の全項目は変化しない
  assert_eq!(proxy.role(), default_proxy.role());
  assert_eq!(proxy.data_center(), default_proxy.data_center());
  assert_eq!(proxy.singleton_identification_interval(), default_proxy.singleton_identification_interval());
  assert_eq!(proxy.buffer_size(), default_proxy.buffer_size());
}

// ── Default trait ────────────────────────────────────────────────────────

#[test]
fn default_equals_new() {
  let via_new = ClusterSingletonConfig::new();
  let via_default = ClusterSingletonConfig::default();
  // Default が new() を委譲していることを確認
  let m1 = via_new.to_manager_config("x");
  let m2 = via_default.to_manager_config("x");
  let p1 = via_new.to_proxy_config("x");
  let p2 = via_default.to_proxy_config("x");
  assert_eq!(m1, m2);
  assert_eq!(p1, p2);
}
