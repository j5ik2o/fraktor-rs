//! Type-specific failure handler for supervision DSL.

#[cfg(test)]
mod tests;

use core::any::TypeId;

use fraktor_actor_core_rs::core::kernel::actor::supervision::SupervisorStrategyConfig;

/// Associates a specific error type with a supervisor strategy.
///
/// Used by [`Supervise::on_failure_of`](crate::dsl::Supervise::on_failure_of) to build
/// type-discriminated supervision chains.
#[derive(Clone, Debug)]
pub struct FailureHandler {
  type_id:   TypeId,
  type_name: &'static str,
  strategy:  SupervisorStrategyConfig,
}

impl FailureHandler {
  /// Creates a new failure handler for the given error type.
  #[must_use]
  pub fn new<E: 'static>(strategy: impl Into<SupervisorStrategyConfig>) -> Self {
    Self { type_id: TypeId::of::<E>(), type_name: core::any::type_name::<E>(), strategy: strategy.into() }
  }

  /// Returns the registered type identity.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the human-readable type name.
  #[must_use]
  pub const fn type_name(&self) -> &'static str {
    self.type_name
  }

  /// Returns the associated supervisor strategy.
  #[must_use]
  pub const fn strategy(&self) -> &SupervisorStrategyConfig {
    &self.strategy
  }
}
