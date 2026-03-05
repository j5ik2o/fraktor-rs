//! Builder that mirrors Fraktor's `Behaviors.supervise` DSL.

use crate::core::{supervision::SupervisorStrategyConfig, typed::behavior::Behavior};

/// Fluent helper returned by [`crate::core::typed::Behaviors::supervise`].
pub struct Supervise<M>
where
  M: Send + Sync + 'static, {
  behavior: Behavior<M>,
}

impl<M> Supervise<M>
where
  M: Send + Sync + 'static,
{
  pub(crate) const fn new(behavior: Behavior<M>) -> Self {
    Self { behavior }
  }

  /// Applies the provided supervisor strategy to the wrapped behavior so that any children
  /// spawned from it inherit the declared supervision policy.
  ///
  /// Accepts [`SupervisorStrategy`](crate::core::supervision::SupervisorStrategy),
  /// [`BackoffSupervisorStrategy`](crate::core::supervision::BackoffSupervisorStrategy),
  /// or [`SupervisorStrategyConfig`] directly.
  #[must_use]
  pub fn on_failure(self, strategy: impl Into<SupervisorStrategyConfig>) -> Behavior<M> {
    self.behavior.with_supervisor_strategy(strategy)
  }
}
