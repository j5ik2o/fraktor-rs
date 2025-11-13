//! Typed scheduler facade bridging typed APIs to the untyped scheduler.

use core::time::Duration;

use crate::{
  RuntimeToolbox,
  messaging::AnyMessageGeneric,
  scheduler::{self, DispatcherSenderShared, Scheduler, SchedulerError, SchedulerHandle},
  typed::actor_prim::TypedActorRefGeneric,
};

/// Provides typed helpers that delegate to the canonical scheduler APIs.
pub struct TypedScheduler<'a, TB: RuntimeToolbox + 'static> {
  scheduler: &'a mut Scheduler<TB>,
}

impl<'a, TB: RuntimeToolbox + 'static> TypedScheduler<'a, TB> {
  /// Creates a typed facade from the underlying scheduler.
  #[must_use]
  pub fn new(scheduler: &'a mut Scheduler<TB>) -> Self {
    Self { scheduler }
  }

  /// Returns a mutable reference to the underlying scheduler (primarily for testing).
  #[allow(dead_code)]
  pub(crate) fn inner(&mut self) -> &mut Scheduler<TB> {
    self.scheduler
  }

  /// Schedules a typed message for one-shot delivery.
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
    scheduler::api::schedule_once(
      self.scheduler,
      delay,
      receiver_untyped,
      AnyMessageGeneric::new(message),
      dispatcher,
      sender_untyped,
    )
  }

  /// Schedules a typed message at a fixed rate.
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
    scheduler::api::schedule_at_fixed_rate(
      self.scheduler,
      initial_delay,
      interval,
      receiver_untyped,
      AnyMessageGeneric::new(message),
      dispatcher,
      sender_untyped,
    )
  }

  /// Schedules a typed message with fixed delay semantics.
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
    scheduler::api::schedule_with_fixed_delay(
      self.scheduler,
      initial_delay,
      delay,
      receiver_untyped,
      AnyMessageGeneric::new(message),
      dispatcher,
      sender_untyped,
    )
  }
}

#[cfg(test)]
mod tests;
