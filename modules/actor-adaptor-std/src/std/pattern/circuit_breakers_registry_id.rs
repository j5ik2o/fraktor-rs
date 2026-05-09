//! Extension identifier for the standard circuit-breaker registry.

use fraktor_actor_core_kernel_rs::{actor::extension::ExtensionId, system::ActorSystem};

use super::circuit_breakers_registry::CircuitBreakersRegistry;

#[cfg(all(test, feature = "test-support"))]
mod tests;

/// Identifier used to register [`CircuitBreakersRegistry`] on an actor system.
#[derive(Clone, Copy, Debug, Default)]
pub struct CircuitBreakersRegistryId;

impl CircuitBreakersRegistryId {
  /// Creates the default circuit-breaker registry identifier.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ExtensionId for CircuitBreakersRegistryId {
  type Ext = CircuitBreakersRegistry;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    CircuitBreakersRegistry::from_actor_system(system)
  }
}
