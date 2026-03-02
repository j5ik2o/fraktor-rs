//! Typed scheduler facade bridging typed APIs to the untyped scheduler.

use core::time::Duration;

use crate::core::{
  messaging::AnyMessage,
  scheduler::{DispatcherSenderShared, Scheduler, SchedulerCommand, SchedulerError, SchedulerHandle},
  typed::actor::TypedActorRef,
};

mod scheduler_context;
#[cfg(test)]
mod tests;
mod typed_scheduler_guard;
mod typed_scheduler_shared;

pub use scheduler_context::TypedSchedulerContext;
pub use typed_scheduler_guard::TypedSchedulerGuard;
pub use typed_scheduler_shared::TypedSchedulerShared;

/// Provides typed helpers that delegate to the canonical scheduler APIs.
pub struct TypedScheduler<'a> {
  scheduler: &'a mut Scheduler,
}

impl<'a> TypedScheduler<'a> {
  /// Creates a typed facade from the underlying scheduler.
  #[must_use]
  pub const fn new(scheduler: &'a mut Scheduler) -> Self {
    Self { scheduler }
  }

  /// Returns a mutable reference to the underlying scheduler (primarily for testing).
  #[allow(dead_code)]
  pub(crate) const fn inner(&mut self) -> &mut Scheduler {
    self.scheduler
  }

  /// Schedules a typed message for one-shot delivery.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_once<M>(
    &mut self,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_once(delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessage::new(message),
      dispatcher,
      sender: sender_untyped,
    })
  }

  /// Schedules a typed message at a fixed rate.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_at_fixed_rate<M>(
    &mut self,
    initial_delay: Duration,
    interval: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_at_fixed_rate(initial_delay, interval, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessage::new(message),
      dispatcher,
      sender: sender_untyped,
    })
  }

  /// Schedules a typed message with fixed delay semantics.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_with_fixed_delay<M>(
    &mut self,
    initial_delay: Duration,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_with_fixed_delay(initial_delay, delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessage::new(message),
      dispatcher,
      sender: sender_untyped,
    })
  }
}
