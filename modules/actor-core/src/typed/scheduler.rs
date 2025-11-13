//! Typed scheduler facade bridging typed APIs to the untyped scheduler.

use core::time::Duration;

use crate::{
  RuntimeToolbox,
  messaging::AnyMessageGeneric,
  scheduler::{DispatcherSenderShared, Scheduler, SchedulerCommand, SchedulerError, SchedulerHandle},
  typed::actor_prim::TypedActorRefGeneric,
};

#[cfg(test)]
mod tests;

/// Provides typed helpers that delegate to the canonical scheduler APIs.
pub struct TypedScheduler<'a, TB: RuntimeToolbox + 'static> {
  scheduler: &'a mut Scheduler<TB>,
}

impl<'a, TB: RuntimeToolbox + 'static> TypedScheduler<'a, TB> {
  /// Creates a typed facade from the underlying scheduler.
  #[must_use]
  pub const fn new(scheduler: &'a mut Scheduler<TB>) -> Self {
    Self { scheduler }
  }

  /// Returns a mutable reference to the underlying scheduler (primarily for testing).
  #[allow(dead_code)]
  pub(crate) const fn inner(&mut self) -> &mut Scheduler<TB> {
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRefGeneric::into_untyped);
    self.scheduler.schedule_once(delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessageGeneric::new(message),
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRefGeneric::into_untyped);
    self.scheduler.schedule_at_fixed_rate(initial_delay, interval, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessageGeneric::new(message),
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    let receiver_untyped = receiver.into_untyped();
    let sender_untyped = sender.map(TypedActorRefGeneric::into_untyped);
    self.scheduler.schedule_with_fixed_delay(initial_delay, delay, SchedulerCommand::SendMessage {
      receiver: receiver_untyped,
      message: AnyMessageGeneric::new(message),
      dispatcher,
      sender: sender_untyped,
    })
  }
}
