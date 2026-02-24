//! Cross-cutting concern interceptor for typed behaviors.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  error::ActorError,
  typed::{actor::TypedActorContextGeneric, behavior::Behavior, behavior_signal::BehaviorSignal},
};

/// Intercepts messages and signals before they reach the wrapped behavior.
///
/// This enables transparent cross-cutting concerns such as logging,
/// monitoring, or message filtering without modifying the inner behavior.
#[allow(clippy::type_complexity)]
pub trait BehaviorInterceptor<M, TB = NoStdToolbox>: Send + Sync
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  /// Called when the wrapped behavior starts.
  ///
  /// The default delegates directly to the `start` callback.
  ///
  /// # Errors
  ///
  /// Returns an error if the interceptor or inner behavior fails.
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    start: &mut dyn FnMut(&mut TypedActorContextGeneric<'_, M, TB>) -> Result<Behavior<M, TB>, ActorError>,
  ) -> Result<Behavior<M, TB>, ActorError> {
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
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContextGeneric<'_, M, TB>, &M) -> Result<Behavior<M, TB>, ActorError>,
  ) -> Result<Behavior<M, TB>, ActorError> {
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
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(
      &mut TypedActorContextGeneric<'_, M, TB>,
      &BehaviorSignal,
    ) -> Result<Behavior<M, TB>, ActorError>,
  ) -> Result<Behavior<M, TB>, ActorError> {
    target(ctx, signal)
  }
}
