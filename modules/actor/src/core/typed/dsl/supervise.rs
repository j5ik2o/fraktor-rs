//! Builder that mirrors Fraktor's `Behaviors.supervise` DSL.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use super::failure_handler::FailureHandler;
use crate::core::{kernel::actor::supervision::SupervisorStrategyConfig, typed::behavior::Behavior};

/// Fluent helper returned by [`crate::core::typed::dsl::Behaviors::supervise`].
pub struct Supervise<M>
where
  M: Send + Sync + 'static, {
  behavior: Behavior<M>,
  handlers: Vec<FailureHandler>,
}

impl<M> Supervise<M>
where
  M: Send + Sync + 'static,
{
  pub(crate) const fn new(behavior: Behavior<M>) -> Self {
    Self { behavior, handlers: Vec::new() }
  }

  /// Applies the provided supervisor strategy to the wrapped behavior so that any children
  /// spawned from it inherit the declared supervision policy.
  ///
  /// Accepts [`SupervisorStrategy`](crate::core::kernel::actor::supervision::SupervisorStrategy),
  /// [`BackoffSupervisorStrategy`](crate::core::kernel::actor::supervision::BackoffSupervisorStrategy),
  /// or [`SupervisorStrategyConfig`] directly.
  #[must_use]
  pub fn on_failure(self, strategy: impl Into<SupervisorStrategyConfig>) -> Behavior<M> {
    if self.handlers.is_empty() {
      return self.behavior.with_supervisor_strategy(strategy);
    }
    let behavior = self.behavior;
    let handlers = self.handlers;
    let composed = Self::build_composed_strategy(handlers, strategy.into());
    behavior.with_supervisor_strategy(composed)
  }

  /// Registers a type-specific failure handler.
  ///
  /// When the error's source type matches `E`, the provided strategy is used instead of the
  /// fallback strategy passed to [`on_failure`](Self::on_failure).
  #[must_use]
  pub fn on_failure_of<E: 'static>(mut self, strategy: impl Into<SupervisorStrategyConfig>) -> Self {
    self.handlers.push(FailureHandler::new::<E>(strategy));
    self
  }

  fn build_composed_strategy(
    handlers: Vec<FailureHandler>,
    fallback: SupervisorStrategyConfig,
  ) -> SupervisorStrategyConfig {
    let composed = crate::core::kernel::actor::supervision::SupervisorStrategy::with_decider(move |error| {
      for handler in &handlers {
        if error.reason().source_type_id() == Some(handler.type_id()) {
          return handler.strategy().decide(error);
        }
      }
      fallback.decide(error)
    });
    SupervisorStrategyConfig::Standard(composed)
  }
}
