//! Builder that mirrors Fraktor's `Behaviors.supervise` DSL.

use crate::core::{supervision::SupervisorStrategy, typed::behavior::Behavior};

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

  /// Applies the provided [`SupervisorStrategy`] to the wrapped behavior so that any children
  /// spawned from it inherit the declared supervision policy.
  #[must_use]
  pub fn on_failure(self, strategy: SupervisorStrategy) -> Behavior<M> {
    self.behavior.with_supervisor_strategy(strategy)
  }
}
