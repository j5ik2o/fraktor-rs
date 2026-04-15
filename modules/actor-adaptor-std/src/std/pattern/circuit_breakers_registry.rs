//! Actor-system scoped registry for shared circuit breakers.

extern crate alloc;
extern crate std;

use alloc::collections::BTreeMap;
use std::{collections::HashMap, string::ToString};

use fraktor_actor_core_rs::core::kernel::{
  actor::{extension::Extension, setup::CircuitBreakerSettings},
  pattern::{CircuitBreaker, CircuitBreakerShared},
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::std::time::StdClock;

#[cfg(test)]
mod tests;

/// Registry that returns a shared circuit breaker per logical name.
///
/// The registry is installed as an actor-system extension so every caller in
/// the same system resolves the same breaker instance for the same key.
pub struct CircuitBreakersRegistry {
  default_settings: CircuitBreakerSettings,
  named_settings:   BTreeMap<String, CircuitBreakerSettings>,
  breakers:         SharedLock<HashMap<String, CircuitBreakerShared<StdClock>>>,
}

impl CircuitBreakersRegistry {
  /// Creates an empty registry with the built-in default breaker settings.
  #[must_use]
  pub fn new() -> Self {
    Self::with_settings(CircuitBreakerSettings::default())
  }

  /// Creates a registry with the supplied default settings.
  #[must_use]
  pub fn with_settings(default_settings: CircuitBreakerSettings) -> Self {
    Self {
      default_settings,
      named_settings: BTreeMap::new(),
      breakers: SharedLock::new_with_driver::<DefaultMutex<_>>(HashMap::new()),
    }
  }

  /// Creates a registry by resolving settings from the provided actor system.
  #[must_use]
  pub fn from_actor_system(system: &ActorSystem) -> Self {
    Self {
      default_settings: system.default_circuit_breaker_settings(),
      named_settings:   system.named_circuit_breaker_settings(),
      breakers:         SharedLock::new_with_driver::<DefaultMutex<_>>(HashMap::new()),
    }
  }

  /// Registers a named circuit-breaker override.
  #[must_use]
  pub fn with_named_settings(mut self, id: impl Into<String>, settings: CircuitBreakerSettings) -> Self {
    self.named_settings.insert(id.into(), settings);
    self
  }

  fn circuit_breaker_settings(&self, id: &str) -> CircuitBreakerSettings {
    self.named_settings.get(id).copied().unwrap_or(self.default_settings)
  }

  /// Returns the shared circuit breaker bound to `id`.
  #[must_use]
  pub fn get(&self, id: &str) -> CircuitBreakerShared<StdClock> {
    let settings = self.circuit_breaker_settings(id);
    self.breakers.with_write(|breakers| {
      breakers.entry(id.to_string()).or_insert_with(|| Self::create_circuit_breaker(settings)).clone()
    })
  }

  fn create_circuit_breaker(settings: CircuitBreakerSettings) -> CircuitBreakerShared<StdClock> {
    let breaker = CircuitBreaker::new_with_clock(settings.max_failures(), settings.reset_timeout(), StdClock);
    CircuitBreakerShared::new(breaker)
  }
}

impl Default for CircuitBreakersRegistry {
  fn default() -> Self {
    Self::new()
  }
}

impl Extension for CircuitBreakersRegistry {}
