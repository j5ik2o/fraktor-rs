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
fn rejects_zero_max_failures() {
  let result = panic::catch_unwind(|| CircuitBreakerConfig::new(0, Duration::from_secs(1)));

  assert!(result.is_err());
}
