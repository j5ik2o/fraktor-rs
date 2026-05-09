//! Signal-only interceptor specialization for typed behaviors.

#[cfg(test)]
mod tests;

use fraktor_actor_core_kernel_rs::actor::error::ActorError;

use crate::{Behavior, actor::TypedActorContext, message_and_signals::BehaviorSignal};

type StartTarget<'a, M> = dyn FnMut(&mut TypedActorContext<'_, M>) -> Result<Behavior<M>, ActorError> + 'a;
type SignalTarget<'a, M> =
  dyn FnMut(&mut TypedActorContext<'_, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError> + 'a;

/// Specialization of [`BehaviorInterceptor`] for signal-only interception.
pub trait BehaviorSignalInterceptor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  /// Called when the wrapped behavior starts.
  ///
  /// # Errors
  ///
  /// Returns an error if the wrapped behavior fails.
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    start: &mut StartTarget<'_, M>,
  ) -> Result<Behavior<M>, ActorError> {
    start(ctx)
  }

  /// Called when the wrapped behavior receives a signal.
  ///
  /// # Errors
  ///
  /// Returns an error if the wrapped behavior fails.
  fn around_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
    target: &mut SignalTarget<'_, M>,
  ) -> Result<Behavior<M>, ActorError> {
    target(ctx, signal)
  }
}
