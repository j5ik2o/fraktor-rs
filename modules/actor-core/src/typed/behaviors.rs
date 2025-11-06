//! Functional builders for typed behaviors.

use crate::{
  RuntimeToolbox,
  error::ActorError,
  typed::{actor_prim::TypedActorContextGeneric, behavior::Behavior, behavior_signal::BehaviorSignal},
};

/// Provides Pekko-inspired helpers for constructing [`Behavior`] instances.
pub struct Behaviors;

impl Behaviors {
  /// Returns a directive that keeps the current behavior.
  #[must_use]
  pub const fn same<M, TB>() -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Behavior::same()
  }

  /// Returns a directive that stops the actor.
  #[must_use]
  pub const fn stopped<M, TB>() -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Behavior::stopped()
  }

  /// Returns a behavior that ignores incoming messages.
  #[must_use]
  pub const fn ignore<M, TB>() -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Behavior::ignore()
  }

  /// Defers behavior creation until the actor is started, allowing access to the context.
  pub fn setup<M, TB, F>(factory: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>) -> Behavior<M, TB> + Send + Sync + 'static, {
    Behavior::from_signal_handler(move |ctx, signal| match signal {
      | BehaviorSignal::Started => Ok(factory(ctx)),
      | _ => Ok(Behavior::same()),
    })
  }

  /// Creates a behavior that handles typed messages and can return the next behavior.
  pub fn receive_message<M, TB, F>(handler: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &M) -> Result<Behavior<M, TB>, ActorError>
      + Send
      + Sync
      + 'static, {
    Behavior::from_message_handler(handler)
  }

  /// Creates a behavior that only reacts to signals.
  pub fn receive_signal<M, TB, F>(handler: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &BehaviorSignal) -> Result<Behavior<M, TB>, ActorError>
      + Send
      + Sync
      + 'static, {
    Behavior::from_signal_handler(handler)
  }
}
