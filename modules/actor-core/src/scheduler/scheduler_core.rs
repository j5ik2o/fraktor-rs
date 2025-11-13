//! Core scheduler implementation placeholder.

use alloc::vec::Vec;
use core::{i32, num::NonZeroU64, time::Duration};

use fraktor_utils_core_rs::{
  sync::ArcShared,
  time::{SchedulerTickHandle, TimerEntry, TimerHandleId, TimerInstant, TimerWheel, TimerWheelConfig},
};
use hashbrown::HashMap;

use super::{
  diagnostics::{DeterministicEvent, SchedulerDiagnostics, SchedulerDiagnosticsEvent, SchedulerDiagnosticsSubscription},
  cancellable_registry::CancellableRegistry,
  command::SchedulerCommand,
  config::SchedulerConfig,
  dump::{SchedulerDump, SchedulerDumpJob},
  error::SchedulerError,
  execution_batch::ExecutionBatch,
  fixed_delay_context::FixedDelayContext,
  fixed_rate_context::FixedRateContext,
  handle::SchedulerHandle,
  metrics::SchedulerMetrics,
  mode::SchedulerMode,
  periodic_batch_decision::PeriodicBatchDecision,
  task_run::{TaskRunEntry, TaskRunHandle, TaskRunOnClose, TaskRunPriority, TaskRunQueue, TaskRunSummary},
  warning::SchedulerWarning,
};
use crate::RuntimeToolbox;

const DEFAULT_DRIFT_BUDGET_PCT: u8 = 5;

/// Scheduler responsible for registering delayed and periodic jobs.
pub struct Scheduler<TB: RuntimeToolbox> {
  toolbox:      TB,
  config:       SchedulerConfig,
  wheel:        TimerWheel<ScheduledPayload>,
  registry:     CancellableRegistry,
  metrics:      SchedulerMetrics,
  warnings:     Vec<SchedulerWarning>,
  next_handle:  u64,
  jobs:         HashMap<u64, ScheduledJob<TB>>,
  current_tick: u64,
  closed:       bool,
  task_runs:    TaskRunQueue,
  task_run_seq: u64,
  task_run_capacity: usize,
  shutting_down: bool,
  diagnostics:  SchedulerDiagnostics,
}

#[allow(dead_code)]
struct ScheduledJob<TB: RuntimeToolbox> {
  handle:        SchedulerHandle,
  wheel_id:      TimerHandleId,
  mode:          SchedulerMode,
  periodic:      Option<PeriodicContext>,
  command:       SchedulerCommand<TB>,
  deadline_tick: u64,
}

#[derive(Clone, Copy, Debug)]
struct ScheduledPayload {
  handle_id: u64,
}

enum PeriodicContext {
  FixedRate(FixedRateContext),
  FixedDelay(FixedDelayContext),
}

impl PeriodicContext {
  fn build_batch(&mut self, now: u64, handle_id: u64) -> PeriodicBatchDecision {
    match self {
      Self::FixedRate(context) => context.build_batch(now, handle_id),
      Self::FixedDelay(context) => context.build_batch(now, handle_id),
    }
  }

  const fn next_deadline_ticks(&self) -> u64 {
    match self {
      Self::FixedRate(context) => context.next_deadline_ticks(),
      Self::FixedDelay(context) => context.next_deadline_ticks(),
    }
  }
}

enum BatchPreparation {
  Ready(ExecutionBatch),
  Cancelled,
}

impl<TB: RuntimeToolbox> Scheduler<TB> {
  /// Creates a scheduler backed by the provided toolbox.
  #[must_use]
  pub fn new(toolbox: TB, config: SchedulerConfig) -> Self {
    let timer_config = TimerWheelConfig::from_profile(config.profile(), config.resolution(), DEFAULT_DRIFT_BUDGET_PCT);
    let wheel = TimerWheel::new(timer_config);
    Self {
      toolbox,
      config,
      wheel,
      registry: CancellableRegistry::default(),
      metrics: SchedulerMetrics::default(),
      warnings: Vec::new(),
      next_handle: 1,
      jobs: HashMap::new(),
      current_tick: 0,
      closed: false,
      task_runs: TaskRunQueue::new(),
      task_run_seq: 0,
      task_run_capacity: config.task_run_capacity(),
      shutting_down: false,
      diagnostics: SchedulerDiagnostics::with_capacity(config.diagnostics_capacity()),
    }
  }

  /// Returns scheduler tick resolution.
  #[must_use]
  pub fn resolution(&self) -> Duration {
    self.config.resolution()
  }

  /// Returns a snapshot of the current scheduler metrics.
  #[must_use]
  pub fn metrics(&self) -> SchedulerMetrics {
    self.metrics
  }

  /// Returns recorded scheduler warnings.
  #[must_use]
  pub fn warnings(&self) -> &[SchedulerWarning] {
    &self.warnings
  }

  /// Enables deterministic logging with the provided capacity.
  pub fn enable_deterministic_log(&mut self, capacity: usize) {
    self.diagnostics.enable_deterministic_log(capacity);
  }

  /// Returns the diagnostics snapshot.
  #[must_use]
  pub fn diagnostics(&self) -> &SchedulerDiagnostics {
    &self.diagnostics
  }

  /// Subscribes to the diagnostics stream.
  pub fn subscribe_diagnostics(&mut self, capacity: usize) -> SchedulerDiagnosticsSubscription {
    self.diagnostics.subscribe(capacity.max(1))
  }

  /// Produces a diagnostic dump of the scheduler state.
  #[must_use]
  pub fn dump(&self) -> SchedulerDump {
    let mut jobs = Vec::with_capacity(self.jobs.len());
    for job in self.jobs.values() {
      let next_tick = job.periodic.as_ref().map(PeriodicContext::next_deadline_ticks);
      jobs.push(SchedulerDumpJob::new(job.handle.raw(), job.mode, job.deadline_tick, next_tick));
    }
    SchedulerDump::new(self.config.resolution(), self.current_tick, self.metrics, jobs, self.warnings.clone())
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
  pub fn cancel(&mut self, handle: &SchedulerHandle) -> bool {
    if let Some(entry) = self.registry.get(handle.raw()) {
      if !entry.try_cancel() {
        return false;
      }
      if let Some(job) = self.jobs.remove(&handle.raw()) {
        let _ = self.wheel.cancel(job.wheel_id);
      }
      self.registry.remove(handle.raw());
      self.metrics.decrement_active();
      self.metrics.increment_dropped();
      self.record_cancel_event(handle.raw());
      true
    } else {
      false
    }
  }

  /// Prevents future scheduling requests and runs registered shutdown tasks.
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.shutdown_with_tasks()
  }

  /// Registers a shutdown task with the specified priority.
  pub fn register_on_close(
    &mut self,
    task: ArcShared<dyn TaskRunOnClose>,
    priority: TaskRunPriority,
  ) -> Result<TaskRunHandle, SchedulerError> {
    if self.task_runs.len() >= self.task_run_capacity {
      return Err(SchedulerError::TaskRunCapacityExceeded);
    }
    let handle = TaskRunHandle::new(self.task_run_seq);
    self.task_run_seq = self.task_run_seq.wrapping_add(1);
    self.task_runs.push(TaskRunEntry::new(priority, self.task_run_seq, handle, task));
    Ok(handle)
  }

  /// Shuts down the scheduler, running registered on-close tasks.
  pub fn shutdown_with_tasks(&mut self) -> TaskRunSummary {
    if self.shutting_down {
      return TaskRunSummary::default();
    }
    self.shutting_down = true;
    self.closed = true;
    self.cancel_all_jobs();
    self.run_task_queue()
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
      let handle_id = payload.handle_id;
      let Some(mut job) = self.jobs.remove(&handle_id) else {
        continue;
      };
      let Some(cancellable) = self.registry.get(handle_id) else {
        continue;
      };

      match self.prepare_batch(&mut job, handle_id) {
        | BatchPreparation::Cancelled => {
          cancellable.force_cancel();
          self.registry.remove(handle_id);
          self.metrics.decrement_active();
          continue;
        },
        | BatchPreparation::Ready(batch) => {
          if !cancellable.try_begin_execute() {
            self.metrics.increment_dropped();
            if cancellable.is_cancelled() {
              self.registry.remove(handle_id);
              self.metrics.decrement_active();
            }
            continue;
          }

          self.execute_command(&job.command, &batch);
          self.record_fire_event(handle_id, batch);
          executed += 1;

          if job.periodic.is_some() {
            if self.reschedule_job(&mut job).is_ok() {
              cancellable.reset_to_scheduled();
              self.jobs.insert(handle_id, job);
            } else {
              cancellable.mark_completed();
              self.registry.remove(handle_id);
              self.metrics.decrement_active();
            }
          } else {
            cancellable.mark_completed();
            self.registry.remove(handle_id);
            self.metrics.decrement_active();
          }
        },
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
  pub fn command_for_test(&self, handle: &SchedulerHandle) -> Option<&SchedulerCommand<TB>> {
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
    let handle = self.next_handle();
    let deadline = self.deadline_from_ticks(delay_ticks);
    let wheel_id = self.enqueue_timer(handle.raw(), deadline)?;
    let entry = handle.entry();
    entry.mark_scheduled();
    self.registry.register(handle.raw(), entry);
    self.metrics.increment_active();
    let periodic = self.build_periodic_context(mode, period, deadline.ticks())?;
    let job = ScheduledJob { handle: handle.clone(), wheel_id, mode, periodic, command, deadline_tick: deadline.ticks() };
    self.jobs.insert(handle.raw(), job);
    self.record_scheduled_event(handle.raw(), deadline, mode);
    Ok(handle)
  }

  fn build_periodic_context(
    &self,
    mode: SchedulerMode,
    period: Option<Duration>,
    start_tick: u64,
  ) -> Result<Option<PeriodicContext>, SchedulerError> {
    match mode {
      | SchedulerMode::OneShot => Ok(None),
      | SchedulerMode::FixedRate => {
        let period_duration = period.ok_or(SchedulerError::InvalidDelay)?;
        let ticks = self.duration_to_ticks(period_duration)?;
        let period_ticks = NonZeroU64::new(ticks).ok_or(SchedulerError::InvalidDelay)?;
        let policy = self.config.fixed_rate_policy();
        Ok(Some(PeriodicContext::FixedRate(FixedRateContext::new(
          start_tick,
          period_ticks,
          policy.backlog_limit(),
          policy.burst_threshold(),
        ))))
      },
      | SchedulerMode::FixedDelay => {
        let period_duration = period.ok_or(SchedulerError::InvalidDelay)?;
        let ticks = self.duration_to_ticks(period_duration)?;
        let period_ticks = NonZeroU64::new(ticks).ok_or(SchedulerError::InvalidDelay)?;
        let policy = self.config.fixed_delay_policy();
        Ok(Some(PeriodicContext::FixedDelay(FixedDelayContext::new(
          start_tick,
          period_ticks,
          policy.backlog_limit(),
          policy.burst_threshold(),
        ))))
      },
    }
  }

  fn prepare_batch(&mut self, job: &mut ScheduledJob<TB>, handle_id: u64) -> BatchPreparation {
    match job.periodic.as_mut() {
      | Some(context) => match context.build_batch(self.current_tick, handle_id) {
        | PeriodicBatchDecision::Execute { batch, warning } => {
          if let Some(warning) = warning {
            self.record_warning(warning);
          }
          BatchPreparation::Ready(batch)
        },
        | PeriodicBatchDecision::Cancel { warning } => {
          self.record_warning(warning);
          self.record_cancel_event(handle_id);
          BatchPreparation::Cancelled
        },
      },
      | None => BatchPreparation::Ready(ExecutionBatch::oneshot()),
    }
  }

  fn cancel_all_jobs(&mut self) {
    let handles: Vec<u64> = self.jobs.keys().copied().collect();
    for handle_id in handles {
      if let Some(entry) = self.registry.remove(handle_id) {
        entry.force_cancel();
        self.metrics.decrement_active();
      }
      if let Some(job) = self.jobs.remove(&handle_id) {
        let _ = self.wheel.cancel(job.wheel_id);
      }
      self.record_cancel_event(handle_id);
    }
  }

  fn run_task_queue(&mut self) -> TaskRunSummary {
    let mut summary = TaskRunSummary::default();
    while let Some(entry) = self.task_runs.pop() {
      match entry.task.run() {
        | Ok(()) => summary.executed_tasks = summary.executed_tasks.saturating_add(1),
        | Err(_) => {
          summary.failed_tasks = summary.failed_tasks.saturating_add(1);
          self.record_warning(SchedulerWarning::TaskRunFailed { handle_id: entry.handle.id() });
        },
      }
    }
    summary
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

  fn deadline_from_absolute(&self, ticks: u64) -> TimerInstant {
    TimerInstant::from_ticks(ticks, self.config.resolution())
  }

  fn next_handle(&mut self) -> SchedulerHandle {
    let handle = SchedulerHandle::new(self.next_handle);
    self.next_handle = self.next_handle.wrapping_add(1).max(1);
    handle
  }

  fn enqueue_timer(&mut self, handle_id: u64, deadline: TimerInstant) -> Result<TimerHandleId, SchedulerError> {
    let payload = ScheduledPayload { handle_id };
    let entry = TimerEntry::oneshot(deadline, payload);
    self.wheel.schedule(entry).map_err(|_| SchedulerError::CapacityExceeded)
  }

  fn reschedule_job(&mut self, job: &mut ScheduledJob<TB>) -> Result<(), SchedulerError> {
    let next_tick = job
      .periodic
      .as_ref()
      .map(PeriodicContext::next_deadline_ticks)
      .ok_or(SchedulerError::CapacityExceeded)?;
    let deadline = self.deadline_from_absolute(next_tick);
    job.wheel_id = self.enqueue_timer(job.handle.raw(), deadline)?;
    job.deadline_tick = deadline.ticks();
    Ok(())
  }

  fn execute_command(&self, command: &SchedulerCommand<TB>, batch: &ExecutionBatch) {
    match command {
      | SchedulerCommand::Noop => {},
      | SchedulerCommand::SendMessage { receiver, message, .. } => {
        let _ = receiver.tell(message.clone());
      },
      | SchedulerCommand::RunRunnable { runnable, .. } => {
        runnable.run(batch);
      },
    }
  }

  fn record_scheduled_event(&mut self, handle_id: u64, deadline: TimerInstant, mode: SchedulerMode) {
    self
      .diagnostics
      .record(DeterministicEvent::Scheduled { handle_id, scheduled_tick: self.current_tick, deadline_tick: deadline.ticks() });
    self.publish_stream_with_drop(SchedulerDiagnosticsEvent::Scheduled { handle_id, deadline_tick: deadline.ticks(), mode });
  }

  fn record_fire_event(&mut self, handle_id: u64, batch: ExecutionBatch) {
    self.diagnostics.record(DeterministicEvent::Fired { handle_id, fired_tick: self.current_tick, batch });
    self.publish_stream_with_drop(SchedulerDiagnosticsEvent::Fired { handle_id, fired_tick: self.current_tick, batch });
  }

  fn record_cancel_event(&mut self, handle_id: u64) {
    self.diagnostics.record(DeterministicEvent::Cancelled { handle_id, cancelled_tick: self.current_tick });
    self.publish_stream_with_drop(SchedulerDiagnosticsEvent::Cancelled { handle_id, cancelled_tick: self.current_tick });
  }

  fn publish_stream_with_drop(&mut self, event: SchedulerDiagnosticsEvent) {
    if self.diagnostics.publish_stream_event(event) {
      self
        .warnings
        .push(SchedulerWarning::DiagnosticsDropped { dropped: 1, capacity: self.config.diagnostics_capacity() });
    }
  }

  fn record_warning(&mut self, warning: SchedulerWarning) {
    let stream_warning = warning.clone();
    self.warnings.push(warning);
    self.publish_stream_with_drop(SchedulerDiagnosticsEvent::Warning { warning: stream_warning });
  }
}
