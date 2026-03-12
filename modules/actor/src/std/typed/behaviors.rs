//! Std-specific typed behavior helpers that wrap the core DSL.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use crate::{
  core::{
    error::ActorError,
    typed::{
      Behavior, BehaviorInterceptor, BehaviorSignal, Behaviors as CoreBehaviors, StashBuffer, Supervise,
      actor::TypedActorContext as CoreTypedActorContext,
    },
  },
  std::typed::{LogOptions, actor::TypedActorContext},
};

/// Interceptor that logs every received message through `tracing`.
struct LogMessagesInterceptor {
  options: LogOptions,
}

impl<M> BehaviorInterceptor<M, M> for LogMessagesInterceptor
where
  M: Send + Sync + core::fmt::Debug + 'static,
{
  fn around_receive(
    &mut self,
    ctx: &mut CoreTypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut CoreTypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    log_received_message(&self.options, ctx.pid(), message);
    target(ctx, message)
  }
}

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

  /// Creates a behavior using a bounded stash helper.
  ///
  /// This mirrors Pekko's `Behaviors.withStash`.
  #[must_use]
  pub fn with_stash<M, F>(capacity: usize, factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn(StashBuffer<M>) -> Behavior<M> + Send + Sync + 'static, {
    CoreBehaviors::with_stash(capacity, factory)
  }

  /// Creates a behavior that handles typed messages using the std context.
  #[must_use]
  pub fn receive_message<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'_, 'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    CoreBehaviors::receive_message(move |ctx, message| with_std_ctx(ctx, |std_ctx| handler(std_ctx, message)))
  }

  /// Creates a behavior that replies to the current sender and keeps the same behavior.
  #[must_use]
  pub fn receive_and_reply<M, R, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    R: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'_, 'a, M>, &M) -> Result<R, ActorError> + Send + Sync + 'static, {
    CoreBehaviors::receive_and_reply(move |ctx, message| with_std_ctx(ctx, |std_ctx| handler(std_ctx, message)))
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

  /// Wraps a behavior so that every received message is logged via
  /// `tracing::debug!`.
  ///
  /// This mirrors Pekko's `Behaviors.logMessages`. The message type must
  /// implement [`Debug`](core::fmt::Debug) so it can be formatted in the log
  /// output.
  #[must_use]
  pub fn log_messages<M>(behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + core::fmt::Debug + 'static, {
    Self::log_messages_with_opts(LogOptions::default(), behavior)
  }

  /// Wraps a behavior so that every received message is logged using `opts`.
  #[must_use]
  pub fn log_messages_with_opts<M>(opts: LogOptions, behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + core::fmt::Debug + 'static, {
    CoreBehaviors::intercept_behavior(move || Box::new(LogMessagesInterceptor { options: opts.clone() }), behavior)
  }
}

fn with_std_ctx<'a, M, R, F>(ctx: &mut CoreTypedActorContext<'a, M>, f: F) -> R
where
  M: Send + Sync + 'static,
  F: FnOnce(&mut TypedActorContext<'_, 'a, M>) -> R, {
  let mut wrapped = TypedActorContext::from_core_mut(ctx);
  f(&mut wrapped)
}

fn log_received_message<M>(options: &LogOptions, pid: crate::core::actor::Pid, message: &M)
where
  M: core::fmt::Debug, {
  if !options.enabled() {
    return;
  }

  match options.logger_name() {
    | Some(logger_name) => match options.level() {
      | tracing::Level::TRACE => tracing::trace!(actor = %pid, logger_name, ?message, "received message"),
      | tracing::Level::DEBUG => tracing::debug!(actor = %pid, logger_name, ?message, "received message"),
      | tracing::Level::INFO => tracing::info!(actor = %pid, logger_name, ?message, "received message"),
      | tracing::Level::WARN => tracing::warn!(actor = %pid, logger_name, ?message, "received message"),
      | tracing::Level::ERROR => tracing::error!(actor = %pid, logger_name, ?message, "received message"),
    },
    | None => match options.level() {
      | tracing::Level::TRACE => tracing::trace!(actor = %pid, ?message, "received message"),
      | tracing::Level::DEBUG => tracing::debug!(actor = %pid, ?message, "received message"),
      | tracing::Level::INFO => tracing::info!(actor = %pid, ?message, "received message"),
      | tracing::Level::WARN => tracing::warn!(actor = %pid, ?message, "received message"),
      | tracing::Level::ERROR => tracing::error!(actor = %pid, ?message, "received message"),
    },
  }
}
