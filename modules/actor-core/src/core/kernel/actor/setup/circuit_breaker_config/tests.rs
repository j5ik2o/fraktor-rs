use core::time::Duration;

use crate::core::kernel::actor::setup::CircuitBreakerConfig;

#[test]
fn default_matches_pekko_registry_defaults() {
  let config = CircuitBreakerConfig::default();

  assert_eq!(config.max_failures(), 5);
  assert_eq!(config.reset_timeout(), Duration::from_secs(30));
}

#[test]
#[should_panic(expected = "max_failures must be greater than zero")]
fn rejects_zero_max_failures() {
  let _ = CircuitBreakerConfig::new(0, Duration::from_secs(1));
}
