//! Functional builders for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::supervise::Supervise;
use crate::core::{
  error::ActorError,
  messaging::AnyMessage,
  typed::{
    actor::{TypedActorContext, TypedActorRef},
    behavior::{Behavior, BehaviorDirective},
    behavior_interceptor::BehaviorInterceptor,
    behavior_signal::BehaviorSignal,
    stash_buffer::StashBuffer,
    timer_scheduler::{TimerScheduler, TimerSchedulerShared},
  },
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
    // Best-effort: ignore send failures to the monitor.
    let _ = self.monitor_ref.tell(message.clone());
    target(ctx, message)
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
    Behavior::from_signal_handler(move |ctx, signal| match signal {
      | BehaviorSignal::Started => Ok(factory(ctx)),
      | _ => Ok(Behavior::same()),
    })
  }

  /// Creates a behavior using a bounded stash helper.
  ///
  /// This mirrors Pekko's `Behaviors.withStash`.
  #[must_use]
  pub fn with_stash<M, F>(capacity: usize, factory: F) -> Behavior<M>
  where
    M: Send + Sync + 'static,
    F: Fn(StashBuffer<M>) -> Behavior<M> + Send + Sync + 'static, {
    Self::setup(move |_ctx| factory(StashBuffer::new(capacity)))
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

  /// Wraps a behavior so that spawned children inherit a declarative
  /// [`SupervisorStrategy`](crate::core::supervision::SupervisorStrategy).
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
  /// Call `.lock()` on the handle to obtain mutable access to the timer scheduler.
  pub fn with_timers<M, F>(factory: F) -> Behavior<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn(TimerSchedulerShared<M>) -> Behavior<M> + Send + Sync + 'static, {
    Self::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      let scheduler = ctx.system().scheduler();
      let timers = TimerScheduler::new(self_ref, scheduler);
      let mutex = RuntimeMutex::new(timers);
      let shared = ArcShared::new(mutex);
      let shared_for_stop = shared.clone();
      factory(shared).compose_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Stopped => {
          shared_for_stop.lock().cancel_all();
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
    Behavior::from_signal_handler(move |ctx, signal| match signal {
      | BehaviorSignal::Started => {
        let mut interceptor = interceptor_factory();
        let mut inner = behavior_factory();

        let started_result =
          interceptor.around_start(ctx, &mut |ctx| inner.handle_signal(ctx, &BehaviorSignal::Started))?;
        if apply_intercepted_directive(&mut inner, started_result).is_err() {
          return Ok(Behavior::stopped());
        }

        let state = InterceptState { interceptor, inner };
        let mutex = RuntimeMutex::new(state);
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

  /// Wraps a behavior so that every received message is cloned and sent to a
  /// monitor actor.
  ///
  /// This mirrors Pekko's `Behaviors.monitor`. The monitor receives a copy of
  /// each message on a best-effort basis — delivery failures are silently
  /// ignored.
  pub fn monitor<M, F>(monitor_ref: TypedActorRef<M>, behavior_factory: F) -> Behavior<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let monitor = monitor_ref;
    Self::intercept(move || Box::new(MonitorInterceptor { monitor_ref: monitor.clone() }), behavior_factory)
  }
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
