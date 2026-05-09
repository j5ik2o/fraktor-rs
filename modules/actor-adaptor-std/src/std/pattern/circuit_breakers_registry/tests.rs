extern crate std;

use core::time::Duration;

use fraktor_actor_core_kernel_rs::{actor::setup::CircuitBreakerConfig, pattern::CircuitBreakerState};
use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::CircuitBreakersRegistry;

#[tokio::test]
async fn get_reuses_breaker_for_same_name() {
  let registry = CircuitBreakersRegistry::new();
  let first = registry.get("payments");
  let second = registry.get("payments");

  let call_result = first.call(|| async { Err::<(), &'static str>("boom") }).await;

  assert!(call_result.is_err());
  assert_eq!(first.failure_count(), 1);
  assert_eq!(second.failure_count(), 1);
}

#[tokio::test]
async fn get_creates_independent_breakers_for_different_names() {
  let registry = CircuitBreakersRegistry::new();
  let payments = registry.get("payments");
  let inventory = registry.get("inventory");

  let call_result = payments.call(|| async { Err::<(), &'static str>("boom") }).await;

  assert!(call_result.is_err());
  assert_eq!(payments.failure_count(), 1);
  assert_eq!(inventory.failure_count(), 0);
  assert_eq!(inventory.state(), CircuitBreakerState::Closed);
}

#[test]
fn get_resolves_named_override_before_creating_breaker() {
  let registry = CircuitBreakersRegistry::with_config(CircuitBreakerConfig::new(3, Duration::from_secs(10)))
    .with_named_config("payments", CircuitBreakerConfig::new(7, Duration::from_secs(45)));

  let payments = registry.get("payments");
  let inventory = registry.get("inventory");

  assert_eq!(payments.with_read(|breaker| breaker.max_failures()), 7);
  assert_eq!(payments.with_read(|breaker| breaker.reset_timeout()), Duration::from_secs(45));
  assert_eq!(inventory.with_read(|breaker| breaker.max_failures()), 3);
  assert_eq!(inventory.with_read(|breaker| breaker.reset_timeout()), Duration::from_secs(10));
}
