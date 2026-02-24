//! Actor-scoped timer management inspired by Pekko's `TimerScheduler`.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::time::Duration;

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::ArcShared,
};
use hashbrown::HashMap;

use crate::core::{
  scheduler::{SchedulerError, SchedulerHandle},
  typed::{actor::TypedActorRefGeneric, scheduler::TypedSchedulerShared, timer_key::TimerKey},
};

/// Manages keyed timers scoped to a single actor.
///
/// Each key can have at most one active timer. Starting a new timer
/// with an existing key cancels the previous timer first. All timers
/// are cancelled when the actor stops.
pub struct TimerSchedulerGeneric<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  self_ref:  TypedActorRefGeneric<M, TB>,
  scheduler: TypedSchedulerShared<TB>,
  entries:   HashMap<TimerKey, SchedulerHandle, RandomState>,
}

/// Type alias for [`TimerSchedulerGeneric`] with the default [`NoStdToolbox`].
pub type TimerScheduler<M> = TimerSchedulerGeneric<M, NoStdToolbox>;

/// Shared handle for [`TimerSchedulerGeneric`], suitable for use in `Fn` closures.
///
/// Users call `.lock()` (via
/// [`SyncMutexLike`](fraktor_utils_rs::core::sync::sync_mutex_like::SyncMutexLike))
/// to obtain mutable access to the underlying timer scheduler.
pub type TimerSchedulerShared<M, TB = NoStdToolbox> = ArcShared<ToolboxMutex<TimerSchedulerGeneric<M, TB>, TB>>;

impl<M, TB> TimerSchedulerGeneric<M, TB>
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a timer scheduler bound to the provided actor.
  #[must_use]
  pub fn new(self_ref: TypedActorRefGeneric<M, TB>, scheduler: TypedSchedulerShared<TB>) -> Self {
    Self { self_ref, scheduler, entries: HashMap::with_hasher(RandomState::new()) }
  }

  /// Starts a timer that sends `message` to self after each `delay`, with non-compensating
  /// semantics. The first message is sent after `delay`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_with_fixed_delay(
    &mut self,
    key: TimerKey,
    message: M,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    self.start_timer_with_fixed_delay_initial(key, message, delay, delay)
  }

  /// Starts a timer that sends `message` to self after each `delay`, with non-compensating
  /// semantics. The first message is sent after `initial_delay`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_with_fixed_delay_initial(
    &mut self,
    key: TimerKey,
    message: M,
    initial_delay: Duration,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    self.cancel(&key);
    let self_ref = self.self_ref.clone();
    let handle = self
      .scheduler
      .with_write(|guard| guard.schedule_with_fixed_delay(initial_delay, delay, self_ref, message, None, None))?;
    self.entries.insert(key, handle);
    Ok(())
  }

  /// Starts a timer that sends `message` to self at each `interval`, with compensating
  /// semantics. The first message is sent after `interval`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_at_fixed_rate(
    &mut self,
    key: TimerKey,
    message: M,
    interval: Duration,
  ) -> Result<(), SchedulerError> {
    self.start_timer_at_fixed_rate_initial(key, message, interval, interval)
  }

  /// Starts a timer that sends `message` to self at each `interval`, with compensating
  /// semantics. The first message is sent after `initial_delay`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_at_fixed_rate_initial(
    &mut self,
    key: TimerKey,
    message: M,
    initial_delay: Duration,
    interval: Duration,
  ) -> Result<(), SchedulerError> {
    self.cancel(&key);
    let self_ref = self.self_ref.clone();
    let handle = self
      .scheduler
      .with_write(|guard| guard.schedule_at_fixed_rate(initial_delay, interval, self_ref, message, None, None))?;
    self.entries.insert(key, handle);
    Ok(())
  }

  /// Starts a one-shot timer that sends `message` to self after `delay`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_single_timer(&mut self, key: TimerKey, message: M, delay: Duration) -> Result<(), SchedulerError> {
    self.cancel(&key);
    let self_ref = self.self_ref.clone();
    let handle = self.scheduler.with_write(|guard| guard.schedule_once(delay, self_ref, message, None, None))?;
    self.entries.insert(key, handle);
    Ok(())
  }

  /// Returns whether a timer with the provided key is currently active.
  #[must_use]
  pub fn is_timer_active(&self, key: &TimerKey) -> bool {
    self.entries.get(key).is_some_and(|h| !h.is_cancelled() && !h.is_completed())
  }

  /// Cancels the timer associated with the provided key.
  pub fn cancel(&mut self, key: &TimerKey) {
    if let Some(handle) = self.entries.remove(key) {
      self.scheduler.with_write(|guard| {
        guard.cancel(&handle);
      });
    }
  }

  /// Cancels all active timers.
  pub fn cancel_all(&mut self) {
    let handles: Vec<SchedulerHandle> = self.entries.drain().map(|(_, h)| h).collect();
    self.scheduler.with_write(|guard| {
      for handle in &handles {
        guard.cancel(handle);
      }
    });
  }
}
