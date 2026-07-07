use core::time::Duration;

use super::{ClusterShardingSettingsError, PassivationStrategy};

#[test]
fn disabled_strategy_is_disabled() {
  assert!(PassivationStrategy::Disabled.is_disabled());
}

#[test]
fn idle_strategy_validates_positive_timeout() {
  let strategy = PassivationStrategy::Idle {
    timeout:        Duration::from_secs(30),
    check_interval: Some(Duration::from_secs(15)),
  };
  assert_eq!(strategy.validate(), Ok(()));
}

#[test]
fn idle_strategy_rejects_zero_timeout() {
  let strategy = PassivationStrategy::Idle { timeout: Duration::ZERO, check_interval: None };
  assert_eq!(strategy.validate(), Err(ClusterShardingSettingsError::ZeroIdleTimeout));
}

#[test]
fn active_limit_rejects_zero_limit() {
  let strategy = PassivationStrategy::ActiveLimit { limit: 0, idle_timeout: None, check_interval: None };
  assert_eq!(strategy.validate(), Err(ClusterShardingSettingsError::ZeroActiveEntityLimit));
}

#[test]
fn lru_strategy_accepts_segmented_proportions() {
  let strategy = PassivationStrategy::Lru {
    limit:                 1_000,
    segmented_proportions: alloc::vec![0.5, 0.5],
    idle_timeout:          None,
    check_interval:        None,
  };
  assert_eq!(strategy.validate(), Ok(()));
}

#[test]
fn lfu_strategy_preserves_dynamic_aging_flag() {
  let strategy = PassivationStrategy::Lfu {
    limit:          500,
    dynamic_aging:  true,
    idle_timeout:   Some(Duration::from_secs(60)),
    check_interval: None,
  };
  assert_eq!(strategy.validate(), Ok(()));
}

#[test]
fn mru_strategy_rejects_zero_idle_timeout() {
  let strategy =
    PassivationStrategy::Mru { limit: 100, idle_timeout: Some(Duration::ZERO), check_interval: None };
  assert_eq!(strategy.validate(), Err(ClusterShardingSettingsError::ZeroIdleTimeout));
}
