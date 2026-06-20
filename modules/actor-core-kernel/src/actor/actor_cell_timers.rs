//! Actor cell timers facet for actor cells.

use alloc::string::String;
use core::{mem, time::Duration};

use fraktor_utils_core_rs::sync::SharedAccess;

use crate::actor::{
  ActorCell,
  messaging::AnyMessage,
  scheduler::{SchedulerCommand, SchedulerError, SchedulerHandle},
};

impl ActorCell {
  fn take_timer_handle(&self, key: &str) -> Option<SchedulerHandle> {
    self.state.with_write(|state| {
      let index = state.timer_handles.iter().position(|(existing, _)| existing == key)?;
      let (_, handle) = state.timer_handles.swap_remove(index);
      Some(handle)
    })
  }

  fn store_timer_handle(&self, key: String, handle: SchedulerHandle) {
    self.state.with_write(|state| {
      state.timer_handles.push((key, handle));
    });
  }

  fn schedule_timer_command(
    &self,
    key: String,
    initial_delay: Duration,
    command: SchedulerCommand,
    interval: Option<Duration>,
    fixed_rate: bool,
  ) -> Result<(), SchedulerError> {
    self.cancel_timer(&key);
    let scheduler = self.system().scheduler();
    let handle = scheduler.with_write(|scheduler| match (interval, fixed_rate) {
      | (Some(duration), true) => scheduler.schedule_at_fixed_rate(initial_delay, duration, command),
      | (Some(duration), false) => scheduler.schedule_with_fixed_delay(initial_delay, duration, command),
      | (None, _) => scheduler.schedule_once(initial_delay, command),
    })?;
    self.store_timer_handle(key, handle);
    Ok(())
  }

  /// Schedules a one-shot timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_single_timer(
    &self,
    key: String,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, delay, command, None, false)
  }

  /// Schedules a fixed-delay timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_fixed_delay_timer(
    &self,
    key: String,
    message: AnyMessage,
    initial_delay: Duration,
    delay: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, initial_delay, command, Some(delay), false)
  }

  /// Schedules a fixed-rate timer associated with `key`.
  ///
  /// # Errors
  ///
  /// Returns an error if the scheduler rejects the request.
  pub(crate) fn schedule_fixed_rate_timer(
    &self,
    key: String,
    message: AnyMessage,
    initial_delay: Duration,
    interval: Duration,
  ) -> Result<(), SchedulerError> {
    let command = SchedulerCommand::SendMessage { receiver: self.actor_ref(), message, sender: None };
    self.schedule_timer_command(key, initial_delay, command, Some(interval), true)
  }

  /// Returns whether the timer associated with `key` is currently active.
  #[must_use]
  pub(crate) fn is_timer_active(&self, key: &str) -> bool {
    self.state.with_read(|state| {
      state
        .timer_handles
        .iter()
        .find(|(existing, _)| existing == key)
        .is_some_and(|(_, handle)| !handle.is_cancelled() && !handle.is_completed())
    })
  }

  /// Cancels the timer associated with `key`.
  pub(crate) fn cancel_timer(&self, key: &str) {
    let Some(handle) = self.take_timer_handle(key) else {
      return;
    };
    self.system().scheduler().with_write(|scheduler| {
      scheduler.cancel(&handle);
    });
  }

  /// Cancels every tracked timer for this actor.
  pub(crate) fn cancel_all_timers(&self) {
    let handles = self.state.with_write(|state| mem::take(&mut state.timer_handles));
    if handles.is_empty() {
      return;
    }
    self.system().scheduler().with_write(|scheduler| {
      for (_, handle) in &handles {
        scheduler.cancel(handle);
      }
    });
  }

  pub(super) fn drop_timer_handles(&self) {
    self.cancel_all_timers();
  }
}
