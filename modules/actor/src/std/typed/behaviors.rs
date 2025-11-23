//! Std-specific typed behavior helpers that wrap the core DSL.

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::{
    error::ActorError,
    typed::{
      BehaviorSignal, Behaviors as CoreBehaviors, actor_prim::TypedActorContextGeneric as CoreTypedActorContext,
    },
  },
  std::typed::{Behavior, Supervise, actor_prim::TypedActorContext},
};

/// Provides Pekko-inspired helpers that operate on std typed contexts.
pub struct Behaviors;

impl Behaviors {
  /// Returns a directive that keeps the current behavior.
  #[must_use]
  pub const fn same<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::same()
  }

  /// Returns a directive that stops the actor.
  #[must_use]
  pub const fn stopped<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::stopped()
  }

  /// Returns a behavior that ignores incoming messages.
  #[must_use]
  pub const fn ignore<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::ignore()
  }

  /// Returns a behavior that marks messages as unhandled.
  #[must_use]
  pub const fn unhandled<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::unhandled()
  }

  /// Returns a behavior that emits unhandled events for every message.
  #[must_use]
  pub const fn empty<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::empty()
  }

  /// Defers behavior creation until the actor is started.
  #[must_use]
  pub fn setup<M, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'_, 'a, M>) -> Behavior<M> + Send + Sync + 'static, {
    CoreBehaviors::setup(move |ctx| with_std_ctx(ctx, |std_ctx| factory(std_ctx)))
  }

  /// Creates a behavior that handles typed messages using the std context.
  #[must_use]
  pub fn receive_message<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'_, 'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    CoreBehaviors::receive_message(move |ctx, message| with_std_ctx(ctx, |std_ctx| handler(std_ctx, message)))
  }

  /// Creates a behavior that only reacts to signals with the std context.
  #[must_use]
  pub fn receive_signal<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'_, 'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
      + Send
      + Sync
      + 'static, {
    CoreBehaviors::receive_signal(move |ctx, signal| with_std_ctx(ctx, |std_ctx| handler(std_ctx, signal)))
  }

  /// Wraps a behavior so that a `SupervisorStrategy` can be assigned declaratively.
  #[must_use]
  pub const fn supervise<M>(behavior: Behavior<M>) -> Supervise<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::supervise(behavior)
  }
}

fn with_std_ctx<'a, M, R, F>(ctx: &mut CoreTypedActorContext<'a, M, StdToolbox>, f: F) -> R
where
  M: Send + Sync + 'static,
  F: FnOnce(&mut TypedActorContext<'_, 'a, M>) -> R, {
  let mut wrapped = TypedActorContext::from_core_mut(ctx);
  f(&mut wrapped)
}
