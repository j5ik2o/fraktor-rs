use core::time::Duration;

use super::RebalanceStrategy;

#[test]
fn default_settings_use_pekko_compatible_limits() {
  let strategy = RebalanceStrategy::least_shards_default();
  let settings = strategy.least_shards_settings().expect("settings");
  assert_eq!(settings.absolute_limit(), 0);
  assert!((settings.relative_limit() - 0.1).abs() < f64::EPSILON);
}

#[test]
fn rebalance_limit_uses_relative_and_absolute_caps() {
  let settings = super::RebalanceStrategySettings::with_limits(5, 0.1);
  assert_eq!(settings.rebalance_limit(100), 5);
  assert_eq!(settings.rebalance_limit(10), 1);
}

#[test]
fn disabled_strategy_reports_zero_interval() {
  let interval = RebalanceStrategy::Disabled.rebalance_interval(Duration::from_secs(10));
  assert_eq!(interval, Duration::ZERO);
}
