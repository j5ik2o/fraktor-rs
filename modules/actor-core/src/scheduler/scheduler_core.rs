//! Core scheduler implementation placeholder.

use core::{i32, time::Duration};

use fraktor_utils_core_rs::time::{
  SchedulerTickHandle, TimerEntry, TimerHandleId, TimerInstant, TimerWheel, TimerWheelConfig,
};
use hashbrown::HashMap;

use super::{
  command::SchedulerCommand, config::SchedulerConfig, error::SchedulerError, execution_batch::ExecutionBatch,
  handle::SchedulerHandle, mode::SchedulerMode,
};
use crate::RuntimeToolbox;

const DEFAULT_DRIFT_BUDGET_PCT: u8 = 5;

/// Scheduler responsible for registering delayed and periodic jobs.
pub struct Scheduler<TB: RuntimeToolbox> {
  #[allow(dead_code)]
  toolbox:      TB,
  config:       SchedulerConfig,
  wheel:        TimerWheel<ScheduledPayload>,
  next_handle:  u64,
  jobs:         HashMap<u64, ScheduledJob<TB>>,
  current_tick: u64,
  closed:       bool,
}

#[allow(dead_code)]
struct ScheduledJob<TB: RuntimeToolbox> {
  handle:       SchedulerHandle,
  wheel_id:     TimerHandleId,
  mode:         SchedulerMode,
  period_ticks: u64,
  command:      SchedulerCommand<TB>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct ScheduledPayload {
  handle: SchedulerHandle,
}

impl<TB: RuntimeToolbox> Scheduler<TB> {
  /// Creates a scheduler backed by the provided toolbox.
  #[must_use]
  pub fn new(toolbox: TB, config: SchedulerConfig) -> Self {
    let timer_config = TimerWheelConfig::from_profile(config.profile(), config.resolution(), DEFAULT_DRIFT_BUDGET_PCT);
    let wheel = TimerWheel::new(timer_config);
    Self { toolbox, config, wheel, next_handle: 1, jobs: HashMap::new(), current_tick: 0, closed: false }
  }

  /// Returns scheduler tick resolution.
  #[must_use]
  pub fn resolution(&self) -> Duration {
    self.config.resolution()
  }

  /// Registers a one-shot job.
  pub fn schedule_once(&mut self, delay: Duration) -> Result<SchedulerHandle, SchedulerError> {
    self.register_job(delay, SchedulerMode::OneShot, None, SchedulerCommand::Noop)
  }

  /// Registers a periodic job with fixed rate.
  pub fn schedule_at_fixed_rate(
    &mut self,
    initial_delay: Duration,
    period: Duration,
  ) -> Result<SchedulerHandle, SchedulerError> {
    self.schedule_at_fixed_rate_with_command(initial_delay, period, SchedulerCommand::Noop)
  }

  /// Registers a periodic job with fixed rate using the provided command.
  pub fn schedule_at_fixed_rate_with_command(
    &mut self,
    initial_delay: Duration,
    period: Duration,
    command: SchedulerCommand<TB>,
  ) -> Result<SchedulerHandle, SchedulerError> {
    self.register_job(initial_delay, SchedulerMode::FixedRate, Some(period), command)
  }

  /// Registers a periodic job with fixed delay.
  pub fn schedule_with_fixed_delay(
    &mut self,
    initial_delay: Duration,
    delay: Duration,
  ) -> Result<SchedulerHandle, SchedulerError> {
    self.schedule_with_fixed_delay_with_command(initial_delay, delay, SchedulerCommand::Noop)
  }

  /// Registers a periodic job with fixed delay using the provided command.
  pub fn schedule_with_fixed_delay_with_command(
    &mut self,
    initial_delay: Duration,
    delay: Duration,
    command: SchedulerCommand<TB>,
  ) -> Result<SchedulerHandle, SchedulerError> {
    self.register_job(initial_delay, SchedulerMode::FixedDelay, Some(delay), command)
  }

  /// Registers a custom command to be executed after the provided delay.
  pub fn schedule_command(
    &mut self,
    delay: Duration,
    command: SchedulerCommand<TB>,
  ) -> Result<SchedulerHandle, SchedulerError> {
    self.register_job(delay, SchedulerMode::OneShot, None, command)
  }

  /// Cancels the job identified by the provided handle.
  pub fn cancel(&mut self, handle: SchedulerHandle) -> bool {
    if let Some(job) = self.jobs.remove(&handle.raw()) {
      let _ = self.wheel.cancel(job.wheel_id);
      true
    } else {
      false
    }
  }

  /// Prevents future scheduling requests.
  pub fn shutdown(&mut self) {
    self.closed = true;
  }

  /// Borrows a tick handle from the toolbox.
  #[allow(dead_code)]
  pub(crate) fn tick_source(&self) -> SchedulerTickHandle<'_> {
    self.toolbox.tick_source()
  }

  /// Runs due timers at the provided instant, returning the number of executed jobs.
  pub fn run_due(&mut self, now: TimerInstant) -> usize {
    self.current_tick = now.ticks();
    let expired = self.wheel.collect_expired(now);
    let mut executed = 0;
    for entry in expired {
      let payload = entry.into_payload();
      if let Some(mut job) = self.jobs.remove(&payload.handle.raw()) {
        self.execute_command(&job.command);
        executed += 1;
        if job.period_ticks > 0 {
          if self.reschedule_job(&mut job).is_ok() {
            self.jobs.insert(job.handle.raw(), job);
          }
        }
      }
    }
    executed
  }

  #[cfg(test)]
  /// Returns the number of registered jobs (testing helper).
  pub fn job_count_for_test(&self) -> usize {
    self.jobs.len()
  }

  #[cfg(test)]
  /// Returns the command associated with the provided handle for testing.
  pub fn command_for_test(&self, handle: SchedulerHandle) -> Option<&SchedulerCommand<TB>> {
    self.jobs.get(&handle.raw()).map(|job| &job.command)
  }

  #[cfg(test)]
  /// Advances the scheduler by the requested ticks (testing helper).
  pub fn run_for_test(&mut self, ticks: u64) {
    self.run_for_ticks(ticks);
  }

  /// Advances the scheduler by the specified number of ticks.
  pub(crate) fn run_for_ticks(&mut self, ticks: u64) {
    let now = self.deadline_from_ticks(ticks);
    let _ = self.run_due(now);
  }

  fn register_job(
    &mut self,
    delay: Duration,
    mode: SchedulerMode,
    period: Option<Duration>,
    command: SchedulerCommand<TB>,
  ) -> Result<SchedulerHandle, SchedulerError> {
    if self.closed {
      return Err(SchedulerError::Closed);
    }
    if self.jobs.len() >= self.config.max_pending_jobs() {
      return Err(SchedulerError::Backpressured);
    }
    let delay_ticks = self.duration_to_ticks(delay)?;
    let period_ticks = match period {
      | Some(duration) => self.duration_to_ticks(duration)?,
      | None => 0,
    };
    let handle = self.next_handle();
    let deadline = self.deadline_from_ticks(delay_ticks);
    let wheel_id = self.enqueue_timer(handle, deadline)?;
    self.jobs.insert(handle.raw(), ScheduledJob { handle, wheel_id, mode, period_ticks, command });
    Ok(handle)
  }

  fn duration_to_ticks(&self, duration: Duration) -> Result<u64, SchedulerError> {
    if duration.is_zero() {
      return Err(SchedulerError::InvalidDelay);
    }
    let resolution_ns = self.config.resolution().as_nanos().max(1);
    let duration_ns = duration.as_nanos();
    let ticks = (duration_ns + resolution_ns - 1) / resolution_ns;
    let max_ticks = i32::MAX as u128;
    if ticks == 0 || ticks > max_ticks {
      return Err(SchedulerError::InvalidDelay);
    }
    Ok(ticks as u64)
  }

  fn deadline_from_ticks(&self, delta: u64) -> TimerInstant {
    let ticks = self.current_tick.saturating_add(delta);
    TimerInstant::from_ticks(ticks, self.config.resolution())
  }

  fn next_handle(&mut self) -> SchedulerHandle {
    let handle = SchedulerHandle::new(self.next_handle);
    self.next_handle = self.next_handle.wrapping_add(1).max(1);
    handle
  }

  fn enqueue_timer(
    &mut self,
    handle: SchedulerHandle,
    deadline: TimerInstant,
  ) -> Result<TimerHandleId, SchedulerError> {
    let payload = ScheduledPayload { handle };
    let entry = TimerEntry::oneshot(deadline, payload);
    self.wheel.schedule(entry).map_err(|_| SchedulerError::CapacityExceeded)
  }

  fn reschedule_job(&mut self, job: &mut ScheduledJob<TB>) -> Result<(), SchedulerError> {
    let deadline = self.deadline_from_ticks(job.period_ticks);
    job.wheel_id = self.enqueue_timer(job.handle, deadline)?;
    Ok(())
  }

  fn execute_command(&self, command: &SchedulerCommand<TB>) {
    let batch = ExecutionBatch::once();
    match command {
      | SchedulerCommand::Noop => {},
      | SchedulerCommand::SendMessage { receiver, message, .. } => {
        let _ = receiver.tell(message.clone());
      },
      | SchedulerCommand::RunRunnable { runnable, .. } => {
        runnable.run(&batch);
      },
    }
  }
}
