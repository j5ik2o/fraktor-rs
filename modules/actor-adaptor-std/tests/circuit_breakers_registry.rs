use core::time::Duration;

use fraktor_actor_adaptor_std_rs::{
  pattern::{CircuitBreakersRegistry, CircuitBreakersRegistryId},
  system::{create_noop_actor_system, create_noop_actor_system_with},
};
use fraktor_actor_core_kernel_rs::{
  actor::{extension::ExtensionId, setup::CircuitBreakerConfig},
  pattern::CircuitBreakerState,
};
use fraktor_utils_core_rs::sync::SharedAccess;

fn assert_extension_id<E: ExtensionId<Ext = CircuitBreakersRegistry>>(_extension_id: &E) {}

#[test]
fn registry_is_available_as_actor_system_extension() {
  // Given: 空の actor system と extension id がある
  let system = create_noop_actor_system();
  let extension_id = CircuitBreakersRegistryId::new();
  assert_extension_id(&extension_id);

  // When: extension を登録して再取得する
  let extended = system.extended();
  assert!(!extended.has_extension(&extension_id));
  let _registry = extended.register_extension(&extension_id);
  let fetched = extended.extension(&extension_id);

  // Then: actor system extension として取得できる
  assert!(extended.has_extension(&extension_id));
  assert!(fetched.is_some());
}

#[tokio::test]
async fn same_name_returns_shared_breaker_state() {
  // Given: 登録済み registry から同名 breaker を 2 回取得する
  let system = create_noop_actor_system();
  let extension_id = CircuitBreakersRegistryId::new();
  let registry = system.extended().register_extension(&extension_id);
  let first = registry.get("payments");
  let second = registry.get("payments");

  // When: 片方で失敗を記録する
  let result = first.call(|| async { Err::<(), &'static str>("boom") }).await;

  // Then: 同名 breaker は状態を共有する
  assert!(result.is_err());
  assert_eq!(first.failure_count(), 1);
  assert_eq!(second.failure_count(), 1);
}

#[tokio::test]
async fn different_names_return_independent_breakers() {
  // Given: 別名 breaker を取得する
  let system = create_noop_actor_system();
  let extension_id = CircuitBreakersRegistryId::new();
  let registry = system.extended().register_extension(&extension_id);
  let payments = registry.get("payments");
  let inventory = registry.get("inventory");

  // When: 一方だけ失敗を記録する
  let result = payments.call(|| async { Err::<(), &'static str>("boom") }).await;

  // Then: 別名 breaker の状態は独立している
  assert!(result.is_err());
  assert_eq!(payments.failure_count(), 1);
  assert_eq!(inventory.failure_count(), 0);
  assert_eq!(inventory.state(), CircuitBreakerState::Closed);
}

#[test]
fn extension_uses_actor_system_default_and_named_settings() {
  let system = create_noop_actor_system_with(|config| {
    config
      .with_default_circuit_breaker_config(CircuitBreakerConfig::new(3, Duration::from_secs(10)))
      .with_named_circuit_breaker_config("payments", CircuitBreakerConfig::new(7, Duration::from_secs(45)))
  });
  let extension_id = CircuitBreakersRegistryId::new();
  let registry = system.extended().register_extension(&extension_id);

  let payments = registry.get("payments");
  let inventory = registry.get("inventory");

  assert_eq!(payments.with_read(|breaker| breaker.max_failures()), 7);
  assert_eq!(payments.with_read(|breaker| breaker.reset_timeout()), Duration::from_secs(45));
  assert_eq!(inventory.with_read(|breaker| breaker.max_failures()), 3);
  assert_eq!(inventory.with_read(|breaker| breaker.reset_timeout()), Duration::from_secs(10));
}
