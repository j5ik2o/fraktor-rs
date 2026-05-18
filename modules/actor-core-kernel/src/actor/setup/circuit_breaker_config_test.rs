use core::time::Duration;
use std::panic;

use crate::actor::setup::CircuitBreakerConfig;

#[test]
fn default_matches_pekko_registry_defaults() {
  let config = CircuitBreakerConfig::default();

  assert_eq!(config.max_failures(), 5);
  assert_eq!(config.reset_timeout(), Duration::from_secs(30));
}

#[test]
fn with_methods_return_updated_copy() {
  let base = CircuitBreakerConfig::default();

  let config = base.with_max_failures(7).with_reset_timeout(Duration::from_secs(45));

  assert_eq!(base.max_failures(), 5);
  assert_eq!(base.reset_timeout(), Duration::from_secs(30));
  assert_eq!(config.max_failures(), 7);
  assert_eq!(config.reset_timeout(), Duration::from_secs(45));
}

#[test]
fn rejects_zero_max_failures() {
  let result = panic::catch_unwind(|| CircuitBreakerConfig::new(0, Duration::from_secs(1)));

  assert!(result.is_err());
}
