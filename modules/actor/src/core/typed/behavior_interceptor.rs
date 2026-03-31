//! Cross-cutting concern interceptor for typed behaviors.

#[cfg(test)]
mod tests;

use crate::core::{
  kernel::actor::error::ActorError,
  typed::{actor::TypedActorContext, behavior::Behavior, behavior_signal::BehaviorSignal},
};

/// Intercepts messages and signals before they reach the wrapped behavior.
///
/// This enables transparent cross-cutting concerns such as logging,
/// monitoring, or message filtering without modifying the inner behavior.
///
/// The `Outer` type parameter represents the external message type received by
/// the interceptor, while `Inner` represents the message type of the wrapped
/// behavior. When `Outer == Inner` (the default), the interceptor acts as a
/// transparent wrapper.
#[allow(clippy::type_complexity)]
pub trait BehaviorInterceptor<Outer, Inner = Outer>: Send + Sync
where
  Outer: Send + Sync + 'static,
  Inner: Send + Sync + 'static, {
  /// Called when the wrapped behavior starts.
  ///
  /// The default delegates directly to the `start` callback.
  ///
  /// # Errors
  ///
  /// Returns an error if the interceptor or inner behavior fails.
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, Outer>,
    start: &mut dyn FnMut(&mut TypedActorContext<'_, Outer>) -> Result<Behavior<Inner>, ActorError>,
  ) -> Result<Behavior<Inner>, ActorError> {
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
    ctx: &mut TypedActorContext<'_, Outer>,
    message: &Outer,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, Outer>, &Outer) -> Result<Behavior<Inner>, ActorError>,
  ) -> Result<Behavior<Inner>, ActorError> {
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
    ctx: &mut TypedActorContext<'_, Outer>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, Outer>, &BehaviorSignal) -> Result<Behavior<Inner>, ActorError>,
  ) -> Result<Behavior<Inner>, ActorError> {
    target(ctx, signal)
  }
}
