//! Public extension point for custom typed behaviors.

use crate::core::{
  kernel::actor::error::ActorError,
  typed::{actor::TypedActorContext, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal},
};

/// OOP-style typed behavior definition that participates in behavior transitions.
///
/// Corresponds to Pekko's `ExtensibleBehavior[T]`. Implement this trait to
/// define actor logic using a mutable struct with `receive` and optionally
/// `receive_signal`.
///
/// Convert to a [`Behavior`] using [`Behaviors::from_extensible`].
///
/// The factory passed to `from_extensible` must be reusable because it can be
/// invoked on the initial start, on restart, and from cloned behaviors.
pub trait ExtensibleBehavior<M>: Send + Sync + 'static
where
  M: Send + Sync + 'static, {
  /// Handles an incoming message and returns the next behavior.
  ///
  /// # Errors
  ///
  /// Returns an error on processing failure.
  fn receive(&mut self, ctx: &mut TypedActorContext<'_, M>, message: &M) -> Result<Behavior<M>, ActorError>;

  /// Handles a lifecycle signal and returns the next behavior.
  ///
  /// The default returns `Behaviors::unhandled()`.
  ///
  /// # Errors
  ///
  /// Returns an error on processing failure.
  fn receive_signal(
    &mut self,
    _ctx: &mut TypedActorContext<'_, M>,
    _signal: &BehaviorSignal,
  ) -> Result<Behavior<M>, ActorError> {
    Ok(Behaviors::unhandled())
  }
}
