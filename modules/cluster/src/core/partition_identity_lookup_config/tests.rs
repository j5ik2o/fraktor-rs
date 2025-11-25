use crate::core::partition_identity_lookup_config::PartitionIdentityLookupConfig;

#[test]
fn default_values_are_correct() {
  // Default トレイト実装でデフォルト値が正しいことを確認
  let config = PartitionIdentityLookupConfig::default();

  // 要件 10.5: デフォルト値
  // - キャッシュ容量: 1024
  // - PID TTL: 300秒
  // - アイドル TTL: 3600秒
  assert_eq!(config.cache_capacity(), 1024);
  assert_eq!(config.pid_ttl_secs(), 300);
  assert_eq!(config.idle_ttl_secs(), 3600);
}

#[test]
fn custom_values_are_preserved() {
  // カスタム値を指定して構造体を作成した場合、値が正しく保持されることを確認
  let config = PartitionIdentityLookupConfig::new(2048, 600, 7200);

  assert_eq!(config.cache_capacity(), 2048);
  assert_eq!(config.pid_ttl_secs(), 600);
  assert_eq!(config.idle_ttl_secs(), 7200);
}

#[test]
fn debug_is_implemented() {
  // Debug トレイトが実装されていることを確認
  let config = PartitionIdentityLookupConfig::default();
  let debug_str = alloc::format!("{:?}", config);
  assert!(debug_str.contains("PartitionIdentityLookupConfig"));
  assert!(debug_str.contains("1024")); // cache_capacity
  assert!(debug_str.contains("300")); // pid_ttl_secs
  assert!(debug_str.contains("3600")); // idle_ttl_secs
}

#[test]
fn clone_is_implemented() {
  // Clone トレイトが実装されていることを確認
  let config = PartitionIdentityLookupConfig::new(512, 120, 1800);
  let cloned = config.clone();

  assert_eq!(config.cache_capacity(), cloned.cache_capacity());
  assert_eq!(config.pid_ttl_secs(), cloned.pid_ttl_secs());
  assert_eq!(config.idle_ttl_secs(), cloned.idle_ttl_secs());
}

#[test]
fn getters_return_correct_values() {
  // 各ゲッターメソッドが正しい値を返すことを確認
  let config = PartitionIdentityLookupConfig::new(100, 50, 200);

  // cache_capacity ゲッター
  assert_eq!(config.cache_capacity(), 100);

  // pid_ttl_secs ゲッター
  assert_eq!(config.pid_ttl_secs(), 50);

  // idle_ttl_secs ゲッター
  assert_eq!(config.idle_ttl_secs(), 200);
}

#[test]
fn zero_values_are_allowed() {
  // ゼロ値が許容されることを確認（境界値テスト）
  let config = PartitionIdentityLookupConfig::new(0, 0, 0);

  assert_eq!(config.cache_capacity(), 0);
  assert_eq!(config.pid_ttl_secs(), 0);
  assert_eq!(config.idle_ttl_secs(), 0);
}

#[test]
fn max_values_are_allowed() {
  // 最大値が許容されることを確認（境界値テスト）
  let config = PartitionIdentityLookupConfig::new(usize::MAX, u64::MAX, u64::MAX);

  assert_eq!(config.cache_capacity(), usize::MAX);
  assert_eq!(config.pid_ttl_secs(), u64::MAX);
  assert_eq!(config.idle_ttl_secs(), u64::MAX);
}
