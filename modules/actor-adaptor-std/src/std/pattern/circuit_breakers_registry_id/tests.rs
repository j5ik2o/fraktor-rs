use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionId, pattern::CircuitBreakerState, system::ActorSystem,
};

use super::CircuitBreakersRegistryId;

#[test]
fn create_extension_returns_empty_registry() {
  let system = ActorSystem::new_empty();
  let extension_id = CircuitBreakersRegistryId::new();

  let registry = extension_id.create_extension(&system);
  let breaker = registry.get("payments");

  assert_eq!(breaker.failure_count(), 0);
  assert_eq!(breaker.state(), CircuitBreakerState::Closed);
}
