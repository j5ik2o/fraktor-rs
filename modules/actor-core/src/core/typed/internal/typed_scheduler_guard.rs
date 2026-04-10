use core::{
  ops::{Deref, DerefMut},
  time::Duration,
};

use crate::core::{
  kernel::actor::scheduler::{Scheduler, SchedulerError, SchedulerHandle},
  typed::{TypedActorRef, internal::TypedScheduler},
};

/// Guard that keeps the scheduler lock and exposes typed scheduling APIs.
pub struct TypedSchedulerGuard<'a> {
  pub(crate) scheduler: &'a mut Scheduler,
}

impl<'a> TypedSchedulerGuard<'a> {
  pub(crate) const fn new(scheduler: &'a mut Scheduler) -> Self {
    Self { scheduler }
  }

  /// Schedules a typed message once while the guard holds the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  pub fn schedule_once<M>(
    &mut self,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_once(delay, receiver, message, sender)
  }

  /// Schedules a typed message at a fixed rate while holding the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  pub fn schedule_at_fixed_rate<M>(
    &mut self,
    initial_delay: Duration,
    interval: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_at_fixed_rate(initial_delay, interval, receiver, message, sender)
  }

  /// Schedules a typed message with fixed delay semantics while holding the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  pub fn schedule_with_fixed_delay<M>(
    &mut self,
    initial_delay: Duration,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_with_fixed_delay(initial_delay, delay, receiver, message, sender)
  }
}

impl Deref for TypedSchedulerGuard<'_> {
  type Target = Scheduler;

  fn deref(&self) -> &Self::Target {
    self.scheduler
  }
}

impl DerefMut for TypedSchedulerGuard<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.scheduler
  }
}
