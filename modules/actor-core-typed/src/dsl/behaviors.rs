//! Functional builders for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::BTreeMap, string::String};
use core::{fmt::Debug, marker::PhantomData};

use fraktor_actor_core_rs::core::kernel::{
  actor::{Pid, error::ActorError, messaging::AnyMessage},
  event::logging::LogLevel,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};

use super::{AbstractBehavior, receive::Receive, supervise::Supervise};
use crate::{
  ExtensibleBehavior, LogOptions, TypedActorRef,
  actor::TypedActorContext,
  behavior::{Behavior, BehaviorDirective},
  behavior_interceptor::BehaviorInterceptor,
  dsl::{StashBuffer, TimerScheduler, TimerSchedulerShared},
  internal::BehaviorSignalInterceptor,
  message_and_signals::BehaviorSignal,
};

/// Internal state for an intercepted behavior.
struct InterceptState<M>
where
  M: Send + Sync + 'static, {
  interceptor: Box<dyn BehaviorInterceptor<M, M>>,
  inner:       Behavior<M>,
}

/// Interceptor that clones every received message to a monitor actor.
struct MonitorInterceptor<M>
where
  M: Send + Sync + Clone + 'static, {
  monitor_ref: TypedActorRef<M>,
}

struct SignalInterceptorAdapter<M>
where
  M: Send + Sync + 'static, {
  interceptor: Box<dyn BehaviorSignalInterceptor<M>>,
  _message:    PhantomData<fn() -> M>,
}

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

impl<M> BehaviorInterceptor<M, M> for MonitorInterceptor<M>
where
  M: Send + Sync + Clone + 'static,
{
  fn around_receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    // best-effort: monitor send failure is non-fatal. dead-letter 観測に加えて
    // interceptor の診断性を保つため warning log を残す。
    if let Err(error) = self.monitor_ref.try_tell(message.clone()) {
      ctx.system().emit_log(
        LogLevel::Warn,
        alloc::format!("monitor interceptor failed to deliver message: {:?}", error),
        Some(ctx.pid()),
        None,
      );
    }
    target(ctx, message)
  }
}

impl<M> BehaviorInterceptor<M, M> for SignalInterceptorAdapter<M>
where
  M: Send + Sync + 'static,
{
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    start: &mut (dyn FnMut(&mut TypedActorContext<'_, M>) -> Result<Behavior<M>, ActorError> + '_),
  ) -> Result<Behavior<M>, ActorError> {
    self.interceptor.around_start(ctx, start)
  }

  fn around_signal(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
    target: &mut (dyn FnMut(&mut TypedActorContext<'_, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError> + '_),
  ) -> Result<Behavior<M>, ActorError> {
    self.interceptor.around_signal(ctx, signal, target)
  }
}

impl<M> BehaviorInterceptor<M, M> for LogMessagesInterceptor
where
  M: Send + Sync + Debug + 'static,
{
  fn around_receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
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
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
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
    ctx: &mut TypedActorContext<'_, M>,
    signal: &BehaviorSignal,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    let span = tracing::info_span!("actor_mdc", actor = %ctx.pid(), mdc = ?self.static_mdc);
    let _guard = span.enter();
    target(ctx, signal)
  }
}

/// Provides Pekko-inspired helpers for constructing [`Behavior`] instances.
pub struct Behaviors;

impl Behaviors {
  /// Returns a directive that keeps the current behavior.
  #[must_use]
  pub const fn same<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    Behavior::same()
  }

  /// Returns a directive that stops the actor.
  #[must_use]
  pub const fn stopped<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    Behavior::stopped()
  }

  /// Returns a behavior that ignores incoming messages.
  #[must_use]
  pub const fn ignore<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
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
  pub const fn unhandled<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
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
  pub const fn empty<M>() -> Behavior<M>
  where
    M: Send + Sync + 'static, {
    Behavior::empty()
  }

  /// Defers behavior creation until the actor is started, allowing access to the context.
  pub fn setup<M, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>) -> Behavior<M> + Send + Sync + 'static, {
    Behavior::from_start_handler(move |ctx| Ok(factory(ctx)))
  }

  /// Creates a behavior using a bounded stash helper.
  ///
  /// This mirrors Pekko's `Behaviors.withStash`.
  ///
  /// This helper does not configure the actor mailbox. Pair it with
  /// `TypedProps::with_stash_mailbox()` so unstash replay uses a deque-capable
  /// mailbox instead of falling back to runtime contract violations.
  #[must_use]
  pub fn with_stash<M, F>(capacity: usize, factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn(StashBuffer<M>) -> Behavior<M> + Send + Sync + 'static, {
    Self::setup(move |_ctx| factory(StashBuffer::new(capacity)))
  }

  /// Creates a receive builder that handles typed messages.
  ///
  /// Unlike [`receive_message`](Self::receive_message) which returns a
  /// [`Behavior`] directly, this method returns an intermediate
  /// [`Receive`] that can be further chained with a signal handler via
  /// [`Receive::receive_signal`].
  ///
  /// Corresponds to Pekko's `Behaviors.receive`.
  pub fn receive<M, F>(handler: F) -> Receive<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    Receive::new(Behavior::from_message_handler(handler))
  }

  /// Creates a behavior that handles typed messages and can return the next behavior.
  pub fn receive_message<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    Behavior::from_message_handler(handler)
  }

  /// Creates a behavior that handles typed messages and replies to the current sender.
  ///
  /// The behavior keeps the current state by returning [`Behaviors::same`].
  /// Use [`Behaviors::receive_message`] when explicit state transitions are needed.
  ///
  /// # Errors
  ///
  /// Returns an error when the handler fails or when no sender is available.
  pub fn receive_and_reply<M, R, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    R: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<R, ActorError> + Send + Sync + 'static, {
    Behavior::from_message_handler(move |ctx, message| {
      let response = handler(ctx, message)?;
      ctx.as_untyped_mut().reply(AnyMessage::new(response)).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behavior::same())
    })
  }

  /// Creates a behavior that handles messages partially.
  ///
  /// The handler returns `Option<Behavior<M>>`. When `None` is returned the
  /// message is treated as unhandled (equivalent to [`Behaviors::unhandled`]).
  /// This mirrors Pekko's `Behaviors.receiveMessagePartial`.
  pub fn receive_message_partial<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Option<Behavior<M>>, ActorError> + Send + Sync + 'static,
  {
    Behavior::from_message_handler(move |ctx, message| match handler(ctx, message)? {
      | Some(behavior) => Ok(behavior),
      | None => Ok(Behavior::unhandled()),
    })
  }

  /// Creates a behavior that handles messages partially.
  ///
  /// The handler returns `Option<Behavior<M>>`. When `None` is returned
  /// the message is treated as unhandled. This mirrors Pekko's
  /// `Behaviors.receivePartial` which accepts a partial message handler.
  ///
  /// To also handle signals, chain `.receive_signal(...)` on the result.
  pub fn receive_partial<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) -> Result<Option<Behavior<M>>, ActorError> + Send + Sync + 'static,
  {
    Self::receive_message_partial(handler)
  }

  /// Creates a behavior that only reacts to signals.
  pub fn receive_signal<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
      + Send
      + Sync
      + 'static, {
    Behavior::from_signal_handler(handler)
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
    M: Send + Sync + Debug + 'static, {
    Self::log_messages_with_opts(LogOptions::default(), behavior)
  }

  /// Wraps a behavior so that every received message is logged using `opts`.
  #[must_use]
  pub fn log_messages_with_opts<M>(opts: LogOptions, behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + Debug + 'static, {
    Self::intercept_behavior(move || Box::new(LogMessagesInterceptor { options: opts.clone() }), behavior)
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
    Self::intercept_behavior(
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
    Self::intercept_behavior(
      move || Box::new(WithMdcInterceptor::<M> { static_mdc: static_mdc.clone(), mdc_for_message: None }),
      behavior,
    )
  }

  /// Wraps a behavior so that spawned children inherit a declarative
  /// [`SupervisorStrategy`](crate::SupervisorStrategy).
  #[must_use]
  pub const fn supervise<M>(behavior: Behavior<M>) -> Supervise<M>
  where
    M: Send + Sync + 'static, {
    Supervise::new(behavior)
  }

  /// Creates a behavior with access to a timer scheduler.
  ///
  /// This mirrors Pekko's `Behaviors.withTimers`. The factory receives a shared
  /// handle to a [`TimerScheduler`] that can be cloned into `Fn` closures.
  /// Call `with_lock` on the handle to obtain mutable access to the timer scheduler.
  pub fn with_timers<M, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn(TimerSchedulerShared<M>) -> Behavior<M> + Send + Sync + 'static, {
    Self::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      let scheduler = ctx.system().raw_scheduler();
      let timers = TimerScheduler::new(self_ref, scheduler);
      let shared = SharedLock::new_with_driver::<DefaultMutex<_>>(timers);
      let shared_for_stop = shared.clone();
      factory(shared).compose_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::PostStop => {
          shared_for_stop.with_lock(|timers| timers.cancel_all());
          Ok(Behavior::same())
        },
        | _ => Ok(Behavior::same()),
      })
    })
  }

  /// Wraps a behavior with a [`BehaviorInterceptor`] for cross-cutting concerns.
  ///
  /// This mirrors Pekko's `Behaviors.intercept`. The interceptor wraps every
  /// message and signal handler call, enabling transparent logging, monitoring,
  /// or message filtering without modifying the inner behavior.
  pub fn intercept<M, I, F>(interceptor_factory: I, behavior_factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    I: Fn() -> Box<dyn BehaviorInterceptor<M, M>> + Send + Sync + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    intercept_inner(interceptor_factory, move || Ok(behavior_factory()))
  }

  /// Wraps a concrete behavior with a [`BehaviorInterceptor`].
  ///
  /// This variant is useful when the caller already has a behavior instance and
  /// still wants to pass through the standard interception path.
  pub fn intercept_behavior<M, I>(interceptor_factory: I, behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    I: Fn() -> Box<dyn BehaviorInterceptor<M, M>> + Send + Sync + 'static, {
    let behavior_template = behavior;
    intercept_inner(interceptor_factory, move || Ok(behavior_template.clone()))
  }

  /// Wraps a behavior with a [`BehaviorSignalInterceptor`] for signal-only concerns.
  pub fn intercept_signal<M, I, F>(interceptor_factory: I, behavior_factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    I: Fn() -> Box<dyn BehaviorSignalInterceptor<M>> + Send + Sync + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    Self::intercept(
      move || Box::new(SignalInterceptorAdapter { interceptor: interceptor_factory(), _message: PhantomData }),
      behavior_factory,
    )
  }

  /// Wraps a behavior to accept a different outer message type.
  ///
  /// This is the factory counterpart of [`Behavior::transform_messages`].
  /// The `mapper` converts incoming `Outer` messages to `Option<Inner>`.
  /// `Some(inner)` is forwarded; `None` means unhandled.
  /// Signals pass through without transformation.
  pub fn transform_messages<Inner, Outer, F>(behavior: Behavior<Inner>, mapper: F) -> Behavior<Outer>
  where
    Inner: Send + Sync + 'static,
    Outer: Send + Sync + 'static,
    F: Fn(&Outer) -> Option<Inner> + Send + Sync + 'static, {
    behavior.transform_messages(mapper)
  }

  /// Wraps a behavior so that every received message is cloned and sent to a
  /// monitor actor.
  ///
  /// This mirrors Pekko's `Behaviors.monitor`. The monitor receives a copy of
  /// each message on a best-effort basis — delivery failures are silently
  /// ignored.
  #[must_use]
  pub fn monitor<M, F>(monitor_ref: TypedActorRef<M>, behavior_factory: F) -> Behavior<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let monitor = monitor_ref;
    Self::intercept(move || Box::new(MonitorInterceptor { monitor_ref: monitor.clone() }), behavior_factory)
  }

  /// Creates a behavior that handles messages without changing state.
  ///
  /// The handler processes the message but always keeps the current behavior.
  /// This is a convenience wrapper around [`receive_message`](Self::receive_message)
  /// for stateless message processing. Corresponds to Pekko's
  /// `Behaviors.receiveMessageWithSame`.
  pub fn receive_message_with_same<M, F>(handler: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &M) + Send + Sync + 'static, {
    Behavior::from_message_handler(move |ctx, msg| {
      handler(ctx, msg);
      Ok(Behavior::same())
    })
  }

  /// Returns a behavior that stops the actor after running a cleanup callback.
  ///
  /// The `post_stop` callback is invoked when the `PostStop` signal is received,
  /// before the actor terminates. Corresponds to Pekko's `Behaviors.stopped(postStop)`.
  pub fn stopped_with_post_stop<M, F>(post_stop: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn() + Send + Sync + 'static, {
    Behavior::stopped().receive_signal(move |_ctx, signal| match signal {
      | BehaviorSignal::PostStop => {
        post_stop();
        Ok(Behavior::stopped())
      },
      | _ => Ok(Behavior::same()),
    })
  }

  /// Wraps a behavior with per-message MDC entries only.
  ///
  /// This is a convenience shorthand for [`with_mdc`](Self::with_mdc)
  /// without static MDC entries. Corresponds to Pekko's
  /// `Behaviors.withMdc(mdcForMessage)(behavior)`.
  #[must_use]
  pub fn with_message_mdc<M, F>(mdc_for_message: F, behavior: Behavior<M>) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn(&M) -> BTreeMap<String, String> + Send + Sync + 'static, {
    Self::with_mdc(BTreeMap::new(), mdc_for_message, behavior)
  }

  /// Creates a behavior from an [`AbstractBehavior`] factory.
  ///
  /// The factory receives the actor context and returns the initial
  /// `AbstractBehavior` instance. Corresponds to Pekko's pattern:
  /// `Behaviors.setup(ctx => new MyBehavior(ctx))`.
  pub fn from_abstract<M, A, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    A: AbstractBehavior<M>,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>) -> A + Send + Sync + 'static, {
    Behaviors::setup(move |ctx| {
      let ab = factory(ctx);
      let shared = SharedLock::new_with_driver::<DefaultMutex<_>>(ab);
      let shared_msg = shared.clone();
      let shared_sig = shared;
      Behaviors::receive_message(move |ctx, msg| shared_msg.with_lock(|behavior| behavior.on_message(ctx, msg)))
        .receive_signal(move |ctx, signal| shared_sig.with_lock(|behavior| behavior.on_signal(ctx, signal)))
    })
  }

  /// Creates a behavior from an [`ExtensibleBehavior`] factory.
  ///
  /// The factory receives the actor context and returns the initial
  /// `ExtensibleBehavior` instance. Corresponds to Pekko's pattern:
  /// `Behaviors.setup(ctx => new MyBehavior(ctx))`.
  pub fn from_extensible<M, E, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    E: ExtensibleBehavior<M>,
    F: for<'a> Fn(&mut TypedActorContext<'a, M>) -> E + Send + Sync + 'static, {
    Behaviors::setup(move |ctx| {
      let extensible = factory(ctx);
      let shared = SharedLock::new_with_driver::<DefaultMutex<_>>(extensible);
      let shared_msg = shared.clone();
      let shared_sig = shared;
      Behaviors::receive_message(move |ctx, msg| shared_msg.with_lock(|behavior| behavior.receive(ctx, msg)))
        .receive_signal(move |ctx, signal| shared_sig.with_lock(|behavior| behavior.receive_signal(ctx, signal)))
    })
  }
}

fn intercept_inner<M, I, F>(interceptor_factory: I, behavior_factory: F) -> Behavior<M>
where
  M: Send + Sync + 'static,
  I: Fn() -> Box<dyn BehaviorInterceptor<M, M>> + Send + Sync + 'static,
  F: Fn() -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
  Behavior::from_start_handler(move |ctx| {
    let mut interceptor = interceptor_factory();
    let mut inner = behavior_factory()?;

    let started_result = interceptor.around_start(ctx, &mut |ctx| inner.handle_start(ctx))?;
    if apply_intercepted_directive(&mut inner, started_result).is_err() {
      return Ok(Behavior::stopped());
    }

    let state = InterceptState { interceptor, inner };
    let shared = SharedLock::new_with_driver::<DefaultMutex<_>>(state);

    let shared_msg = shared.clone();
    let shared_sig = shared;

    Ok(
      Behaviors::receive_message(move |ctx, msg| {
        let mut next_inner = None;
        let next = {
          shared_msg.with_lock(|guard| {
            let InterceptState { interceptor, inner } = guard;
            let next = interceptor.around_receive(ctx, msg, &mut |ctx, msg| inner.handle_message(ctx, msg))?;
            next_inner = Some(next);
            Ok::<(), ActorError>(())
          })?;
          // Safety: `next_inner` is guaranteed to be `Some` here because the
          // `with_lock` closure above sets it before returning `Ok`, and we
          // only reach this line when `?` did not propagate an error.
          #[allow(clippy::expect_used)]
          next_inner.take().expect("interceptor must produce next behavior")
        };
        shared_msg.with_lock(|guard| Ok(resolve_intercepted_directive(&mut guard.inner, next)))
      })
      .receive_signal(move |ctx, signal| {
        let mut next_inner = None;
        let next = {
          shared_sig.with_lock(|guard| {
            let InterceptState { interceptor, inner } = guard;
            let next = interceptor.around_signal(ctx, signal, &mut |ctx, sig| inner.handle_signal(ctx, sig))?;
            next_inner = Some(next);
            Ok::<(), ActorError>(())
          })?;
          // Safety: `next_inner` is guaranteed to be `Some` here because the
          // `with_lock` closure above sets it before returning `Ok`, and we
          // only reach this line when `?` did not propagate an error.
          #[allow(clippy::expect_used)]
          next_inner.take().expect("interceptor must produce next behavior")
        };
        shared_sig.with_lock(|guard| Ok(resolve_intercepted_directive(&mut guard.inner, next)))
      }),
    )
  })
}

/// Applies the behavior directive from an interceptor result to the inner behavior.
///
/// Returns `Err(())` when the inner behavior requests a stop during startup,
/// so the caller can construct `Behavior::stopped()` and propagate it.
fn apply_intercepted_directive<M>(inner: &mut Behavior<M>, next: Behavior<M>) -> Result<(), ()>
where
  M: Send + Sync + 'static, {
  match next.directive() {
    | BehaviorDirective::Active => {
      *inner = next;
      Ok(())
    },
    | BehaviorDirective::Empty => {
      *inner = Behavior::empty();
      Ok(())
    },
    | BehaviorDirective::Stopped => Err(()),
    | _ => Ok(()),
  }
}

/// Resolves the interceptor result into the outer behavior directive.
fn resolve_intercepted_directive<M>(inner: &mut Behavior<M>, next: Behavior<M>) -> Behavior<M>
where
  M: Send + Sync + 'static, {
  match next.directive() {
    | BehaviorDirective::Same | BehaviorDirective::Ignore => Behaviors::same(),
    | BehaviorDirective::Stopped => Behaviors::stopped(),
    | BehaviorDirective::Unhandled => Behaviors::unhandled(),
    | BehaviorDirective::Empty => {
      *inner = Behavior::empty();
      Behaviors::same()
    },
    | BehaviorDirective::Active => {
      *inner = next;
      Behaviors::same()
    },
  }
}

fn log_received_message<M>(options: &LogOptions, pid: Pid, message: &M)
where
  M: Debug, {
  if !options.enabled() {
    return;
  }

  match options.logger_name() {
    | Some(logger_name) => match options.level() {
      | LogLevel::Trace => tracing::trace!(actor = %pid, logger_name, ?message, "received message"),
      | LogLevel::Debug => tracing::debug!(actor = %pid, logger_name, ?message, "received message"),
      | LogLevel::Info => tracing::info!(actor = %pid, logger_name, ?message, "received message"),
      | LogLevel::Warn => tracing::warn!(actor = %pid, logger_name, ?message, "received message"),
      | LogLevel::Error => tracing::error!(actor = %pid, logger_name, ?message, "received message"),
    },
    | None => match options.level() {
      | LogLevel::Trace => tracing::trace!(actor = %pid, ?message, "received message"),
      | LogLevel::Debug => tracing::debug!(actor = %pid, ?message, "received message"),
      | LogLevel::Info => tracing::info!(actor = %pid, ?message, "received message"),
      | LogLevel::Warn => tracing::warn!(actor = %pid, ?message, "received message"),
      | LogLevel::Error => tracing::error!(actor = %pid, ?message, "received message"),
    },
  }
}
