//! Builder that mirrors Fraktor's `Behaviors.supervise` DSL.

use fraktor_utils_core_rs::sync::NoStdToolbox;

use crate::{RuntimeToolbox, supervision::SupervisorStrategy, typed::behavior::Behavior};

/// Fluent helper returned by [`crate::typed::Behaviors::supervise`].
pub struct Supervise<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  behavior: Behavior<M, TB>,
}

impl<M, TB> Supervise<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) const fn new(behavior: Behavior<M, TB>) -> Self {
    Self { behavior }
  }

  /// Applies the provided [`SupervisorStrategy`] to the wrapped behavior so that any children
  /// spawned from it inherit the declared supervision policy.
  #[must_use]
  pub fn on_failure(self, strategy: SupervisorStrategy) -> Behavior<M, TB> {
    self.behavior.with_supervisor_strategy(strategy)
  }
}
