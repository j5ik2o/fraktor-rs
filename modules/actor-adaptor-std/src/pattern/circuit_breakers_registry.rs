//! Actor-system scoped registry for shared circuit breakers.

extern crate alloc;
extern crate std;

use alloc::collections::BTreeMap;
use std::{collections::HashMap, string::ToString};

use fraktor_actor_core_kernel_rs::{
  actor::{extension::Extension, setup::CircuitBreakerConfig},
  pattern::{CircuitBreaker, CircuitBreakerShared},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::time::StdClock;

#[cfg(test)]
#[path = "circuit_breakers_registry_test.rs"]
mod tests;

/// Registry that returns a shared circuit breaker per logical name.
///
/// The registry is installed as an actor-system extension so every caller in
/// the same system resolves the same breaker instance for the same key.
pub struct CircuitBreakersRegistry {
  default_config: CircuitBreakerConfig,
  named_configs:  BTreeMap<String, CircuitBreakerConfig>,
  breakers:       SharedLock<HashMap<String, CircuitBreakerShared<StdClock>>>,
}

impl CircuitBreakersRegistry {
  /// Creates an empty registry with the built-in default breaker configuration.
  #[must_use]
  pub fn new() -> Self {
    Self::with_config(CircuitBreakerConfig::default())
  }

  /// Creates a registry with the supplied default configuration.
  #[must_use]
  pub fn with_config(default_config: CircuitBreakerConfig) -> Self {
    Self {
      default_config,
      named_configs: BTreeMap::new(),
      breakers: SharedLock::new_with_driver::<DefaultMutex<_>>(HashMap::new()),
    }
  }

  /// Creates a registry by resolving configuration from the provided actor system.
  #[must_use]
  pub fn from_actor_system(system: &ActorSystem) -> Self {
    Self {
      default_config: system.default_circuit_breaker_config(),
      named_configs:  system.named_circuit_breaker_config(),
      breakers:       SharedLock::new_with_driver::<DefaultMutex<_>>(HashMap::new()),
    }
  }

  /// Registers a named circuit-breaker override.
  #[must_use]
  pub fn with_named_config(mut self, id: impl Into<String>, config: CircuitBreakerConfig) -> Self {
    self.named_configs.insert(id.into(), config);
    self
  }

  fn circuit_breaker_config(&self, id: &str) -> CircuitBreakerConfig {
    self.named_configs.get(id).copied().unwrap_or(self.default_config)
  }

  /// Returns the shared circuit breaker bound to `id`.
  #[must_use]
  pub fn get(&self, id: &str) -> CircuitBreakerShared<StdClock> {
    let config = self.circuit_breaker_config(id);
    self.breakers.with_write(|breakers| {
      breakers.entry(id.to_string()).or_insert_with(|| Self::create_circuit_breaker(config)).clone()
    })
  }

  fn create_circuit_breaker(config: CircuitBreakerConfig) -> CircuitBreakerShared<StdClock> {
    let breaker = CircuitBreaker::new_with_clock(config.max_failures(), config.reset_timeout(), StdClock);
    CircuitBreakerShared::new(breaker)
  }
}

impl Default for CircuitBreakersRegistry {
  fn default() -> Self {
    Self::new()
  }
}

impl Extension for CircuitBreakersRegistry {}
