//! Cross-cutting concern interceptor for typed behaviors.

#[cfg(test)]
mod tests;

use crate::core::{
  error::ActorError,
  typed::{actor::TypedActorContext, behavior::Behavior, behavior_signal::BehaviorSignal},
};

/// Intercepts messages and signals before they reach the wrapped behavior.
///
/// This enables transparent cross-cutting concerns such as logging,
/// monitoring, or message filtering without modifying the inner behavior.
#[allow(clippy::type_complexity)]
pub trait BehaviorInterceptor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  /// Called when the wrapped behavior starts.
  ///
  /// The default delegates directly to the `start` callback.
  ///
  /// # Errors
  ///
  /// Returns an error if the interceptor or inner behavior fails.
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    start: &mut dyn FnMut(&mut TypedActorContext<'_, M>) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    start(ctx)
  }

  /// Called when the wrapped behavior receives a message.
  ///
  /// The default delegates directly to the `target` callback.
  ///
  /// # Errors
  ///
  /// Returns an error if the interceptor or inner behavior fails.
  fn around_receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    target(ctx, message)
  }

  /// Called when the wrapped behavior receives a signal.
  ///
  /// The default delegates directly to the `target` callback.
  ///
  /// # Errors
  ///
  /// Returns an error if the interceptor or inner behavior fails.
  fn around_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    target(ctx, signal)
  }
}
