//! Typed scheduler facade bridging typed APIs to the untyped scheduler.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_actor_core_rs::actor::{
  messaging::AnyMessage,
  scheduler::{Scheduler, SchedulerCommand, SchedulerError, SchedulerHandle},
};

use crate::TypedActorRef;

/// Provides typed helpers that delegate to the canonical scheduler APIs.
pub(crate) struct TypedScheduler<'a> {
  scheduler: &'a mut Scheduler,
}

impl<'a> TypedScheduler<'a> {
  /// Creates a typed facade from the underlying scheduler.
  #[must_use]
  pub(crate) const fn new(scheduler: &'a mut Scheduler) -> Self {
    Self { scheduler }
  }

  /// Schedules a typed message for one-shot delivery.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  pub(crate) fn schedule_once<M>(
    &mut self,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_once(delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message:  AnyMessage::new(message),
      sender:   sender_untyped,
    })
  }

  /// Schedules a typed message at a fixed rate.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  pub(crate) fn schedule_at_fixed_rate<M>(
    &mut self,
    initial_delay: Duration,
    interval: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_at_fixed_rate(initial_delay, interval, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message:  AnyMessage::new(message),
      sender:   sender_untyped,
    })
  }

  /// Schedules a typed message with fixed delay semantics.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler is not initialized or if scheduling fails.
  pub(crate) fn schedule_with_fixed_delay<M>(
    &mut self,
    initial_delay: Duration,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRef::into_untyped);
    self.scheduler.schedule_with_fixed_delay(initial_delay, delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message:  AnyMessage::new(message),
      sender:   sender_untyped,
    })
  }
}
