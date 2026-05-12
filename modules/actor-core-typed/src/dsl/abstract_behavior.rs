//! OOP-style actor definition for behavior transitions.

#[cfg(test)]
#[path = "abstract_behavior_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::actor::error::ActorError;

use crate::{actor::TypedActorContext, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal};

/// OOP-style actor definition that participates in behavior transitions.
///
/// Corresponds to Pekko's `AbstractBehavior[T]`. Implement this trait
/// to define actor logic using a mutable struct with `on_message` and
/// optionally `on_signal`.
///
/// Convert to a [`Behavior`] using [`Behaviors::from_abstract`].
///
/// The factory passed to `from_abstract` must be reusable because it can be
/// invoked on the initial start, on restart, and from cloned behaviors.
pub trait AbstractBehavior<M>: Send + Sync + 'static
where
  M: Send + Sync + 'static, {
  /// Handles an incoming message and returns the next behavior.
  ///
  /// # Errors
  ///
  /// Returns an error on processing failure.
  fn on_message(&mut self, ctx: &mut TypedActorContext<'_, M>, msg: &M) -> Result<Behavior<M>, ActorError>;

  /// Handles a lifecycle signal and returns the next behavior.
  ///
  /// The default returns `Behaviors::unhandled()`.
  ///
  /// # Errors
  ///
  /// Returns an error on processing failure.
  fn on_signal(
    &mut self,
    _ctx: &mut TypedActorContext<'_, M>,
    _signal: &BehaviorSignal,
  ) -> Result<Behavior<M>, ActorError> {
    Ok(Behaviors::unhandled())
  }
}
