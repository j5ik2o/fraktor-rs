use core::ops::{Deref, DerefMut};

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};

use crate::core::{
  scheduler::{DispatcherSenderShared, Scheduler, SchedulerError, SchedulerHandle},
  typed::{TypedScheduler, actor_prim::TypedActorRefGeneric},
};

type SchedulerMutex<TB> = ToolboxMutex<Scheduler<TB>, TB>;
type SchedulerMutexGuard<'a, TB> = <SchedulerMutex<TB> as SyncMutexLike<Scheduler<TB>>>::Guard<'a>;

/// Guard that keeps the scheduler lock and exposes typed scheduling APIs.
pub struct TypedSchedulerGuard<'a, TB: RuntimeToolbox + 'static> {
  pub(crate) guard: SchedulerMutexGuard<'a, TB>,
}

impl<'a, TB: RuntimeToolbox + 'static> TypedSchedulerGuard<'a, TB> {
  /// Provides a typed scheduler facade scoped to the current lock.
  #[allow(clippy::missing_const_for_fn)] // ロック取得が必要なため const fn にできない
  pub fn scheduler(&'a mut self) -> TypedScheduler<'a, TB> {
    TypedScheduler::new(&mut self.guard)
  }

  /// Returns mutable access to the underlying scheduler for diagnostics.
  /// Executes a closure with a typed scheduler reference while holding the lock.
  pub fn with<F, R>(&mut self, callback: F) -> R
  where
    F: for<'b> FnOnce(&mut TypedScheduler<'b, TB>) -> R, {
    let mut typed = TypedScheduler::new(&mut self.guard);
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(&mut self.guard).schedule_once(delay, receiver, message, dispatcher, sender)
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(&mut self.guard).schedule_at_fixed_rate(
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
    receiver: TypedActorRefGeneric<M, TB>,
    message: M,
    dispatcher: Option<DispatcherSenderShared<TB>>,
    sender: Option<TypedActorRefGeneric<M, TB>>,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    TypedScheduler::new(&mut self.guard).schedule_with_fixed_delay(
      initial_delay,
      delay,
      receiver,
      message,
      dispatcher,
      sender,
    )
  }
}

impl<TB: RuntimeToolbox + 'static> Deref for TypedSchedulerGuard<'_, TB> {
  type Target = Scheduler<TB>;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<TB: RuntimeToolbox + 'static> DerefMut for TypedSchedulerGuard<'_, TB> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}
