//! Actor-scoped timer management inspired by Pekko's `TimerScheduler`.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::{
  hash::{Hash, Hasher},
  time::Duration,
};

use ahash::{AHasher, RandomState};
use fraktor_actor_core_rs::core::kernel::actor::scheduler::{SchedulerError, SchedulerHandle};
use fraktor_utils_core_rs::core::sync::SharedLock;
use hashbrown::HashMap;

use crate::{TypedActorRef, dsl::TimerKey, internal::TypedSchedulerShared};

/// Manages keyed timers scoped to a single actor.
///
/// Each key can have at most one active timer. Starting a new timer
/// with an existing key cancels the previous timer first. All timers
/// are cancelled when the actor stops.
pub struct TimerScheduler<M>
where
  M: Send + Sync + 'static, {
  self_ref:  TypedActorRef<M>,
  scheduler: TypedSchedulerShared,
  entries:   HashMap<TimerKey, SchedulerHandle, RandomState>,
}

/// Shared handle for [`TimerScheduler`], suitable for use in `Fn` closures.
pub type TimerSchedulerShared<M> = SharedLock<TimerScheduler<M>>;

impl<M> TimerScheduler<M>
where
  M: Send + Sync + Clone + 'static,
{
  fn timer_key_for_message(message: &M) -> TimerKey
  where
    M: Hash, {
    let mut hasher = AHasher::default();
    message.hash(&mut hasher);
    TimerKey::new(alloc::format!("{:016x}", hasher.finish()))
  }

  /// Creates a timer scheduler bound to the provided actor.
  #[must_use]
  pub fn new(self_ref: TypedActorRef<M>, scheduler: TypedSchedulerShared) -> Self {
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

  /// Starts a timer using `message` itself as the timer key.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_with_fixed_delay_with_message_key(
    &mut self,
    message: M,
    delay: Duration,
  ) -> Result<(), SchedulerError>
  where
    M: Hash, {
    let key = Self::timer_key_for_message(&message);
    self.start_timer_with_fixed_delay(key, message, delay)
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
      .with_write(|guard| guard.schedule_with_fixed_delay(initial_delay, delay, self_ref, message, None))?;
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

  /// Starts a fixed-rate timer using `message` itself as the timer key.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_timer_at_fixed_rate_with_message_key(
    &mut self,
    message: M,
    interval: Duration,
  ) -> Result<(), SchedulerError>
  where
    M: Hash, {
    let key = Self::timer_key_for_message(&message);
    self.start_timer_at_fixed_rate(key, message, interval)
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
      .with_write(|guard| guard.schedule_at_fixed_rate(initial_delay, interval, self_ref, message, None))?;
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
    let handle = self.scheduler.with_write(|guard| guard.schedule_once(delay, self_ref, message, None))?;
    self.entries.insert(key, handle);
    Ok(())
  }

  /// Starts a one-shot timer using `message` itself as the timer key.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the command.
  pub fn start_single_timer_with_message_key(&mut self, message: M, delay: Duration) -> Result<(), SchedulerError>
  where
    M: Hash, {
    let key = Self::timer_key_for_message(&message);
    self.start_single_timer(key, message, delay)
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
