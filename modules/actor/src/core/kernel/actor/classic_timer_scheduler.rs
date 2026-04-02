//! Classic actor timer facade.

use alloc::string::String;
use core::time::Duration;

use crate::core::kernel::{
  actor::{Pid, messaging::AnyMessage, scheduler::SchedulerError},
  system::ActorSystem,
};

/// Actor-scoped classic timer facade backed by the kernel scheduler.
pub struct ClassicTimerScheduler {
  system: ActorSystem,
  pid:    Pid,
}

impl ClassicTimerScheduler {
  /// Creates a classic timer facade for the running actor.
  #[must_use]
  pub fn new(system: &ActorSystem, pid: Pid) -> Self {
    Self { system: system.clone(), pid }
  }

  fn with_cell<R>(&self, f: impl FnOnce(&crate::core::kernel::actor::ActorCell) -> R) -> Result<R, SchedulerError> {
    let state = self.system.state();
    let Some(cell) = state.cell(&self.pid) else {
      return Err(SchedulerError::ActorUnavailable);
    };
    Ok(f(&cell))
  }

  /// Starts a one-shot timer that sends `message` to self after `delay`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the timer registration.
  pub fn start_single_timer(
    &self,
    key: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    self.with_cell(|cell| cell.schedule_single_timer(key.into(), message, delay))?
  }

  /// Starts a timer that sends `message` to self with fixed-delay semantics.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the timer registration.
  pub fn start_timer_with_fixed_delay(
    &self,
    key: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    self.with_cell(|cell| cell.schedule_fixed_delay_timer(key.into(), message, delay, delay))?
  }

  /// Starts a timer that sends `message` to self with fixed-rate semantics.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the timer registration.
  pub fn start_timer_at_fixed_rate(
    &self,
    key: impl Into<String>,
    message: AnyMessage,
    interval: Duration,
  ) -> Result<(), SchedulerError> {
    self.with_cell(|cell| cell.schedule_fixed_rate_timer(key.into(), message, interval, interval))?
  }

  /// Returns whether the timer registered under `key` is still active.
  #[must_use]
  pub fn is_timer_active(&self, key: &str) -> bool {
    self.with_cell(|cell| cell.is_timer_active(key)).unwrap_or(false)
  }

  /// Cancels the timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error when the backing actor cell is no longer available.
  pub fn cancel(&self, key: &str) -> Result<(), SchedulerError> {
    self.with_cell(|cell| cell.cancel_timer(key))
  }

  /// Cancels every active timer registered for this actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the backing actor cell is no longer available.
  pub fn cancel_all(&self) -> Result<(), SchedulerError> {
    self.with_cell(crate::core::kernel::actor::ActorCell::cancel_all_timers)
  }
}
