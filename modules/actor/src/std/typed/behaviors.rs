//! Std-specific typed behavior helpers that wrap the core DSL.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::BTreeMap, string::String};

use fraktor_utils_rs::core::sync::ArcShared;

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

/// Computes per-message MDC entries for [`WithMdcInterceptor`].
type MdcForMessageFn<M> = dyn Fn(&M) -> BTreeMap<String, String> + Send + Sync;

/// Interceptor that sets tracing span fields for each message and signal.
///
/// Corresponds to Pekko's `WithMdcBehaviorInterceptor`. Static MDC entries
/// are applied to every message and signal. Per-message MDC entries are
/// computed from each message and merged with the static entries.
struct WithMdcInterceptor<M>
where
  M: Send + Sync + 'static, {
  static_mdc:      BTreeMap<String, String>,
  mdc_for_message: Option<Box<MdcForMessageFn<M>>>,
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

impl<M> BehaviorInterceptor<M, M> for WithMdcInterceptor<M>
where
  M: Send + Sync + 'static,
{
  fn around_receive(
    &mut self,
    ctx: &mut CoreTypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut CoreTypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    let mut mdc = self.static_mdc.clone();
    if let Some(ref f) = self.mdc_for_message {
      mdc.extend(f(message));
    }
    let span = tracing::info_span!("actor_mdc", actor = %ctx.pid(), mdc = ?mdc);
    let _guard = span.enter();
    target(ctx, message)
  }

  fn around_signal(
    &mut self,
    ctx: &mut CoreTypedActorContext<'_, M>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(&mut CoreTypedActorContext<'_, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    let span = tracing::info_span!("actor_mdc", actor = %ctx.pid(), mdc = ?self.static_mdc);
    let _guard = span.enter();
    target(ctx, signal)
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

  /// Wraps a behavior with MDC (Mapped Diagnostic Context) support.
  ///
  /// Static MDC entries are applied to all messages and signals. Per-message
  /// MDC entries are computed from each message and override static entries
  /// with the same key.
  ///
  /// In Rust, MDC values are emitted as a `tracing::Span` field, which
  /// subscribers can extract for structured logging. This mirrors Pekko's
  /// `Behaviors.withMdc`.
  #[must_use]
  pub fn with_mdc<M, F>(
    static_mdc: BTreeMap<String, String>,
    mdc_for_message: F,
    behavior: Behavior<M>,
  ) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn(&M) -> BTreeMap<String, String> + Send + Sync + 'static, {
    let shared_fn: ArcShared<MdcForMessageFn<M>> = ArcShared::new(mdc_for_message);
    CoreBehaviors::intercept_behavior(
      move || {
        let fn_clone = shared_fn.clone();
        Box::new(WithMdcInterceptor {
          static_mdc:      static_mdc.clone(),
          mdc_for_message: Some(Box::new(move |msg: &M| fn_clone(msg))),
        })
      },
      behavior,
    )
  }

  /// Wraps a behavior with static-only MDC entries.
  ///
  /// The provided entries are applied as tracing span fields on every
  /// message and signal delivery. This is a convenience shorthand for
  /// [`with_mdc`](Self::with_mdc) without per-message MDC.
  #[must_use]
  pub fn with_static_mdc<M>(static_mdc: BTreeMap<String, String>, behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    CoreBehaviors::intercept_behavior(
      move || Box::new(WithMdcInterceptor::<M> { static_mdc: static_mdc.clone(), mdc_for_message: None }),
      behavior,
    )
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
