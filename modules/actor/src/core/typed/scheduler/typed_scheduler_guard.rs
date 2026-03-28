use core::ops::{Deref, DerefMut};

use crate::core::{
  kernel::scheduler::{DispatcherSenderShared, Scheduler, SchedulerError, SchedulerHandle},
  typed::{actor::TypedActorRef, scheduler::TypedScheduler},
};

/// Guard that keeps the scheduler lock and exposes typed scheduling APIs.
pub struct TypedSchedulerGuard<'a> {
  pub(crate) scheduler: &'a mut Scheduler,
}

impl<'a> TypedSchedulerGuard<'a> {
  pub(crate) const fn new(scheduler: &'a mut Scheduler) -> Self {
    Self { scheduler }
  }

  /// Provides a typed scheduler facade scoped to the current lock.
  #[allow(clippy::missing_const_for_fn)] // ロック取得が必要なため const fn にできない
  pub fn scheduler(&'a mut self) -> TypedScheduler<'a> {
    TypedScheduler::new(self.scheduler)
  }

  /// Returns mutable access to the underlying scheduler for diagnostics.
  /// Executes a closure with a typed scheduler reference while holding the lock.
  pub fn with<F, R>(&mut self, callback: F) -> R
  where
    F: for<'b> FnOnce(&mut TypedScheduler<'b>) -> R, {
    let mut typed = TypedScheduler::new(self.scheduler);
    callback(&mut typed)
  }

  /// Schedules a typed message once while the guard holds the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_once<M>(
    &mut self,
    delay: core::time::Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_once(delay, receiver, message, dispatcher, sender)
  }

  /// Schedules a typed message at a fixed rate while holding the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_at_fixed_rate<M>(
    &mut self,
    initial_delay: core::time::Duration,
    interval: core::time::Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_at_fixed_rate(
      initial_delay,
      interval,
      receiver,
      message,
      dispatcher,
      sender,
    )
  }

  /// Schedules a typed message with fixed delay semantics while holding the lock.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or command enqueue fails.
  #[allow(clippy::too_many_arguments)]
  pub fn schedule_with_fixed_delay<M>(
    &mut self,
    initial_delay: core::time::Duration,
    delay: core::time::Duration,
    receiver: TypedActorRef<M>,
    message: M,
    dispatcher: Option<DispatcherSenderShared>,
    sender: Option<TypedActorRef<M>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(self.scheduler).schedule_with_fixed_delay(
      initial_delay,
      delay,
      receiver,
      message,
      dispatcher,
      sender,
    )
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
