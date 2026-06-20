use core::any::TypeId;

use portable_atomic::Ordering;

use super::RuntimeSupportRegistry;
use crate::actor::setup::CircuitBreakerConfig;

#[test]
fn runtime_support_registry_starts_empty() {
  let mut registry = RuntimeSupportRegistry::noop();

  assert_eq!(registry.next_pid.load(Ordering::Relaxed), 0);
  assert_eq!(registry.clock.load(Ordering::Relaxed), 0);
  assert!(registry.named_circuit_breaker_config.is_empty());
  assert!(registry.ask_futures.drain_ready().is_empty());
  let _factory = registry.invoke_guard_factory.clone();
}

#[test]
fn runtime_support_registry_default_uses_noop_support() {
  let registry = RuntimeSupportRegistry::default();

  assert_eq!(registry.next_pid.load(Ordering::Relaxed), 0);
  assert_eq!(registry.clock.load(Ordering::Relaxed), 0);
  assert!(!registry.extensions.contains_key(&TypeId::of::<usize>()));
  assert_eq!(registry.default_circuit_breaker_config, CircuitBreakerConfig::default());
  assert!(registry.named_circuit_breaker_config.is_empty());
}
