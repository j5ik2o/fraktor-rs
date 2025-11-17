//! Functional builders for typed behaviors.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::supervise::Supervise;
use crate::core::{
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

  /// Returns a behavior that signals a message was not handled.
  ///
  /// This is used to advise the system to reuse the previous behavior, including the hint
  /// that the message has not been handled. This hint may be used by composite behaviors
  /// that delegate (partial) handling to other behaviors.
  ///
  /// Unlike `ignore()`, this will emit an `UnhandledMessage` event to the event stream
  /// for monitoring and debugging purposes.
  #[must_use]
  pub const fn unhandled<M, TB>() -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Behavior::unhandled()
  }

  /// Returns a behavior that treats every incoming message as unhandled.
  ///
  /// This is useful when the actor has reached a state where no more messages are expected,
  /// but the actor has not yet stopped. For example, when waiting for all spawned child
  /// actors to terminate before stopping.
  ///
  /// Unlike `ignore()`, which silently drops messages without logging, `empty()` will
  /// emit an `UnhandledMessage` event to the event stream for every received message,
  /// allowing monitoring and debugging of unexpected messages.
  ///
  /// Unlike `unhandled()`, which reverts to the previous behavior, `empty()` maintains
  /// the empty state indefinitely until explicitly changed.
  #[must_use]
  pub const fn empty<M, TB>() -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Behavior::empty()
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

  /// Wraps a behavior so that spawned children inherit a declarative [`SupervisorStrategy`].
  #[must_use]
  pub const fn supervise<M, TB>(behavior: Behavior<M, TB>) -> Supervise<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Supervise::new(behavior)
  }
}
