use fraktor_actor_core_kernel_rs::{actor::extension::ExtensionId, pattern::CircuitBreakerState};

use super::CircuitBreakersRegistryId;
use crate::system::create_noop_actor_system;

#[test]
fn create_extension_returns_empty_registry() {
  let system = create_noop_actor_system();
  let extension_id = CircuitBreakersRegistryId::new();

  let registry = extension_id.create_extension(&system);
  let breaker = registry.get("payments");

  assert_eq!(breaker.failure_count(), 0);
  assert_eq!(breaker.state(), CircuitBreakerState::Closed);
}
