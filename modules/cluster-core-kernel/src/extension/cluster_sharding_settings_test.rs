use core::time::Duration;

use super::{ClusterShardingSettings, ClusterShardingSettingsError, PassivationStrategy};

#[test]
fn defaults_match_pekko_compatible_values() {
  let settings = ClusterShardingSettings::new();
  assert_eq!(settings.number_of_shards(), 100);
  assert_eq!(settings.role(), None);
  assert_eq!(settings.passivation_strategy(), &PassivationStrategy::Disabled);
  assert!(!settings.remember_entities());
  assert_eq!(settings.hand_over_retry_interval(), Duration::from_secs(1));
  assert_eq!(settings.min_hand_over_retries(), 15);
  assert_eq!(settings.retry_interval(), Duration::from_secs(2));
  assert_eq!(settings.rebalance_interval(), Duration::from_secs(10));
  assert_eq!(settings.hand_off_timeout(), Duration::from_secs(60));
  assert_eq!(settings.shard_region_query_timeout(), Duration::from_secs(3));
  assert_eq!(settings.buffer_size(), 100_000);
  assert_eq!(settings.entity_restart_backoff(), Duration::from_secs(10));
}

#[test]
fn builder_preserves_custom_values() {
  let settings = ClusterShardingSettings::new()
    .with_number_of_shards(256)
    .with_role("sharding")
    .with_passivation_strategy(PassivationStrategy::ActiveLimit {
      limit:          10_000,
      idle_timeout:   Some(Duration::from_secs(120)),
      check_interval: Some(Duration::from_secs(60)),
    })
    .with_remember_entities(false)
    .with_hand_over_retry_interval(Duration::from_millis(500))
    .with_min_hand_over_retries(20)
    .with_retry_interval(Duration::from_secs(5))
    .with_rebalance_interval(Duration::from_secs(30))
    .with_hand_off_timeout(Duration::from_secs(90))
    .with_shard_region_query_timeout(Duration::from_secs(7))
    .with_buffer_size(50_000)
    .with_entity_restart_backoff(Duration::from_secs(15));

  assert_eq!(settings.number_of_shards(), 256);
  assert_eq!(settings.role(), Some("sharding"));
  assert_eq!(settings.hand_over_retry_interval(), Duration::from_millis(500));
  assert_eq!(settings.min_hand_over_retries(), 20);
  assert_eq!(settings.retry_interval(), Duration::from_secs(5));
  assert_eq!(settings.rebalance_interval(), Duration::from_secs(30));
  assert_eq!(settings.hand_off_timeout(), Duration::from_secs(90));
  assert_eq!(settings.shard_region_query_timeout(), Duration::from_secs(7));
  assert_eq!(settings.buffer_size(), 50_000);
  assert_eq!(settings.entity_restart_backoff(), Duration::from_secs(15));
}

#[test]
fn validate_rejects_zero_number_of_shards() {
  let settings = ClusterShardingSettings::new().with_number_of_shards(0);
  assert_eq!(settings.validate(), Err(ClusterShardingSettingsError::ZeroNumberOfShards));
}

#[test]
fn validate_rejects_zero_hand_over_retry_interval() {
  let settings = ClusterShardingSettings::new().with_hand_over_retry_interval(Duration::ZERO);
  assert_eq!(settings.validate(), Err(ClusterShardingSettingsError::ZeroHandOverRetryInterval));
}

#[test]
fn validate_rejects_passivation_with_remember_entities() {
  let settings =
    ClusterShardingSettings::new().with_remember_entities(true).with_passivation_strategy(PassivationStrategy::Idle {
      timeout:        Duration::from_secs(30),
      check_interval: None,
    });
  assert_eq!(settings.validate(), Err(ClusterShardingSettingsError::PassivationWithRememberEntities));
}

#[test]
fn validate_accepts_default_settings() {
  assert_eq!(ClusterShardingSettings::new().validate(), Ok(()));
}
