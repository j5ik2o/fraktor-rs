//! Functional builders for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::supervise::Supervise;
use crate::core::{
  error::ActorError,
  messaging::AnyMessageGeneric,
  typed::{
    actor::TypedActorContextGeneric,
    behavior::{Behavior, BehaviorDirective},
    behavior_interceptor::BehaviorInterceptor,
    behavior_signal::BehaviorSignal,
    stash_buffer::StashBufferGeneric,
    timer_scheduler::{TimerSchedulerGeneric, TimerSchedulerShared},
  },
};

/// Internal state for an intercepted behavior.
struct InterceptState<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  interceptor: Box<dyn BehaviorInterceptor<M, TB>>,
  inner:       Behavior<M, TB>,
}

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

  /// Creates a behavior using a bounded stash helper.
  ///
  /// This mirrors Pekko's `Behaviors.withStash`.
  #[must_use]
  pub fn with_stash<M, TB, F>(capacity: usize, factory: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    F: Fn(StashBufferGeneric<M, TB>) -> Behavior<M, TB> + Send + Sync + 'static, {
    Self::setup(move |_ctx| factory(StashBufferGeneric::new(capacity)))
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

  /// Creates a behavior that handles typed messages and replies to the current sender.
  ///
  /// The behavior keeps the current state by returning [`Behaviors::same`].
  /// Use [`Behaviors::receive_message`] when explicit state transitions are needed.
  ///
  /// # Errors
  ///
  /// Returns an error when the handler fails or when no sender is available.
  pub fn receive_and_reply<M, TB, R, F>(handler: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    R: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    F: for<'a> Fn(&mut TypedActorContextGeneric<'a, M, TB>, &M) -> Result<R, ActorError> + Send + Sync + 'static, {
    Behavior::from_message_handler(move |ctx, message| {
      let response = handler(ctx, message)?;
      ctx
        .as_untyped_mut()
        .reply(AnyMessageGeneric::new(response))
        .map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behavior::same())
    })
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

  /// Wraps a behavior so that spawned children inherit a declarative
  /// [`SupervisorStrategy`](crate::core::supervision::SupervisorStrategy).
  #[must_use]
  pub const fn supervise<M, TB>(behavior: Behavior<M, TB>) -> Supervise<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static, {
    Supervise::new(behavior)
  }

  /// Creates a behavior with access to a timer scheduler.
  ///
  /// This mirrors Pekko's `Behaviors.withTimers`. The factory receives a shared
  /// handle to a [`TimerSchedulerGeneric`] that can be cloned into `Fn` closures.
  /// Call `.lock()` on the handle to obtain mutable access to the timer scheduler.
  pub fn with_timers<M, TB, F>(factory: F) -> Behavior<M, TB>
  where
    M: Send + Sync + Clone + 'static,
    TB: RuntimeToolbox + 'static,
    F: Fn(TimerSchedulerShared<M, TB>) -> Behavior<M, TB> + Send + Sync + 'static, {
    Self::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      let scheduler = ctx.system().scheduler();
      let timers = TimerSchedulerGeneric::new(self_ref, scheduler);
      let mutex = <TB::MutexFamily as SyncMutexFamily>::create(timers);
      let shared = ArcShared::new(mutex);
      let shared_for_stop = shared.clone();
      factory(shared).receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Stopped => {
          shared_for_stop.lock().cancel_all();
          Ok(Behavior::stopped())
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
  pub fn intercept<M, TB, I, F>(interceptor_factory: I, behavior_factory: F) -> Behavior<M, TB>
  where
    M: Send + Sync + 'static,
    TB: RuntimeToolbox + 'static,
    I: Fn() -> Box<dyn BehaviorInterceptor<M, TB>> + Send + Sync + 'static,
    F: Fn() -> Behavior<M, TB> + Send + Sync + 'static, {
    Behavior::from_signal_handler(move |ctx, signal| match signal {
      | BehaviorSignal::Started => {
        let mut interceptor = interceptor_factory();
        let mut inner = behavior_factory();

        let started_result =
          interceptor.around_start(ctx, &mut |ctx| inner.handle_signal(ctx, &BehaviorSignal::Started))?;
        if let Some(stopped) = apply_intercepted_directive(&mut inner, started_result) {
          return Ok(stopped);
        }

        let state = InterceptState { interceptor, inner };
        let mutex = <TB::MutexFamily as SyncMutexFamily>::create(state);
        let shared = ArcShared::new(mutex);

        let shared_msg = shared.clone();
        let shared_sig = shared;

        Ok(
          Behaviors::receive_message(move |ctx, msg| {
            let mut guard = shared_msg.lock();
            let next = {
              let InterceptState { interceptor, inner } = &mut *guard;
              interceptor.around_receive(ctx, msg, &mut |ctx, msg| inner.handle_message(ctx, msg))?
            };
            Ok(resolve_intercepted_directive(&mut guard.inner, next))
          })
          .receive_signal(move |ctx, signal| {
            let mut guard = shared_sig.lock();
            let next = {
              let InterceptState { interceptor, inner } = &mut *guard;
              interceptor.around_signal(ctx, signal, &mut |ctx, sig| inner.handle_signal(ctx, sig))?
            };
            Ok(resolve_intercepted_directive(&mut guard.inner, next))
          }),
        )
      },
      | _ => Ok(Behavior::same()),
    })
  }
}

/// Applies the behavior directive from an interceptor result to the inner behavior.
///
/// Returns `Some(Behavior::stopped())` when the inner behavior requests a stop
/// during startup, so the caller can propagate it instead of continuing.
fn apply_intercepted_directive<M, TB>(
  inner: &mut Behavior<M, TB>,
  next: Behavior<M, TB>,
) -> Option<Behavior<M, TB>>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  match next.directive() {
    | BehaviorDirective::Active => {
      *inner = next;
      None
    },
    | BehaviorDirective::Empty => {
      *inner = Behavior::empty();
      None
    },
    | BehaviorDirective::Stopped => Some(Behavior::stopped()),
    | _ => None,
  }
}

/// Resolves the interceptor result into the outer behavior directive.
fn resolve_intercepted_directive<M, TB>(inner: &mut Behavior<M, TB>, next: Behavior<M, TB>) -> Behavior<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
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
