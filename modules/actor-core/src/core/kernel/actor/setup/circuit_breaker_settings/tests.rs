use core::time::Duration;

use crate::core::kernel::actor::setup::CircuitBreakerSettings;

#[test]
fn default_matches_pekko_registry_defaults() {
  let settings = CircuitBreakerSettings::default();

  assert_eq!(settings.max_failures(), 5);
  assert_eq!(settings.reset_timeout(), Duration::from_secs(30));
}

#[test]
#[should_panic(expected = "max_failures must be greater than zero")]
fn rejects_zero_max_failures() {
  let _ = CircuitBreakerSettings::new(0, Duration::from_secs(1));
}
