//! Runtime support state owned by SystemState.

#[cfg(test)]
#[path = "runtime_support_registry_test.rs"]
mod tests;

use alloc::{boxed::Box, collections::BTreeMap, string::String};

use fraktor_utils_core_rs::sync::ArcShared;
use portable_atomic::AtomicU64;

use super::{AskFutures, Extensions};
use crate::actor::{
  invoke_guard::{InvokeGuardFactory, NoopInvokeGuardFactory},
  setup::CircuitBreakerConfig,
};

/// Owns runtime support state for the actor system.
pub(crate) struct RuntimeSupportRegistry {
  pub(crate) next_pid: AtomicU64,
  pub(crate) clock: AtomicU64,
  pub(crate) ask_futures: AskFutures,
  pub(crate) extensions: Extensions,
  pub(crate) invoke_guard_factory: ArcShared<Box<dyn InvokeGuardFactory>>,
  pub(crate) default_circuit_breaker_config: CircuitBreakerConfig,
  pub(crate) named_circuit_breaker_config: BTreeMap<String, CircuitBreakerConfig>,
}

impl RuntimeSupportRegistry {
  pub(crate) fn new(invoke_guard_factory: ArcShared<Box<dyn InvokeGuardFactory>>) -> Self {
    Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      ask_futures: AskFutures::default(),
      extensions: Extensions::default(),
      invoke_guard_factory,
      default_circuit_breaker_config: CircuitBreakerConfig::default(),
      named_circuit_breaker_config: BTreeMap::new(),
    }
  }

  pub(crate) fn noop() -> Self {
    Self::new(NoopInvokeGuardFactory::shared())
  }
}

impl Default for RuntimeSupportRegistry {
  fn default() -> Self {
    Self::noop()
  }
}
