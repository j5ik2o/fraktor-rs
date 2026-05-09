//! Typed scheduler facade for convenient scheduling APIs.
//!
//! Corresponds to `org.apache.pekko.actor.typed.Scheduler` in the Pekko
//! reference implementation. Wraps the internal [`TypedSchedulerShared`] and
//! provides direct scheduling methods without exposing the lock/guard pattern.

use core::time::Duration;

use fraktor_actor_core_rs::actor::scheduler::{
  Scheduler as KernelScheduler, SchedulerCommand, SchedulerError, SchedulerHandle, SchedulerRunnable,
};
use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{TypedActorRef, internal::TypedSchedulerShared};

/// Typed scheduler facade that exposes scheduling methods directly.
///
/// Corresponds to Pekko's `ActorSystem.scheduler` / `Scheduler` trait.
/// Unlike the internal [`TypedSchedulerShared`], this type provides a
/// user-friendly API where each scheduling method acquires and releases
/// the scheduler lock internally.
///
/// Instances are obtained through [`TypedActorSystem::scheduler()`].
#[derive(Clone)]
pub struct Scheduler {
  inner: TypedSchedulerShared,
}

impl Scheduler {
  /// Creates a new scheduler facade wrapping the given shared handle.
  #[must_use]
  pub(crate) const fn new(inner: TypedSchedulerShared) -> Self {
    Self { inner }
  }

  /// Schedules a single message delivery after the given delay.
  ///
  /// Corresponds to Pekko's `Scheduler.scheduleOnce`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_once<M>(
    &self,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    self.inner.with_write(|guard| guard.schedule_once(delay, receiver, message, None))
  }

  /// Schedules repeated message delivery at a fixed rate.
  ///
  /// Corresponds to Pekko's `Scheduler.scheduleAtFixedRate`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_at_fixed_rate<M>(
    &self,
    initial_delay: Duration,
    interval: Duration,
    receiver: TypedActorRef<M>,
    message: M,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    self.inner.with_write(|guard| guard.schedule_at_fixed_rate(initial_delay, interval, receiver, message, None))
  }

  /// Schedules repeated message delivery with a fixed delay between completions.
  ///
  /// Corresponds to Pekko's `Scheduler.scheduleWithFixedDelay`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_with_fixed_delay<M>(
    &self,
    initial_delay: Duration,
    delay: Duration,
    receiver: TypedActorRef<M>,
    message: M,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    M: Send + Sync + 'static, {
    self.inner.with_write(|guard| guard.schedule_with_fixed_delay(initial_delay, delay, receiver, message, None))
  }

  /// Schedules a runnable once after the given delay.
  ///
  /// Corresponds to Pekko's runnable-oriented `Scheduler.scheduleOnce`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_once_runnable<R>(&self, delay: Duration, runnable: R) -> Result<SchedulerHandle, SchedulerError>
  where
    R: SchedulerRunnable, {
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
    let command = SchedulerCommand::RunRunnable { runnable };
    self.inner.with_write(|guard| KernelScheduler::schedule_once(&mut *guard, delay, command))
  }

  /// Schedules a runnable repeatedly at a fixed rate.
  ///
  /// Corresponds to Pekko's runnable-oriented `Scheduler.scheduleAtFixedRate`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_at_fixed_rate_runnable<R>(
    &self,
    initial_delay: Duration,
    interval: Duration,
    runnable: R,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    R: SchedulerRunnable, {
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
    let command = SchedulerCommand::RunRunnable { runnable };
    self
      .inner
      .with_write(|guard| KernelScheduler::schedule_at_fixed_rate(&mut *guard, initial_delay, interval, command))
  }

  /// Schedules a runnable repeatedly with a fixed delay between completions.
  ///
  /// Corresponds to Pekko's runnable-oriented `Scheduler.scheduleWithFixedDelay`.
  ///
  /// # Errors
  ///
  /// Returns [`SchedulerError`] when the scheduler is not ready or
  /// command enqueue fails.
  pub fn schedule_with_fixed_delay_runnable<R>(
    &self,
    initial_delay: Duration,
    delay: Duration,
    runnable: R,
  ) -> Result<SchedulerHandle, SchedulerError>
  where
    R: SchedulerRunnable, {
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
    let command = SchedulerCommand::RunRunnable { runnable };
    self
      .inner
      .with_write(|guard| KernelScheduler::schedule_with_fixed_delay(&mut *guard, initial_delay, delay, command))
  }
}
