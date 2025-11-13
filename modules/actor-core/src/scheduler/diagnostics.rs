//! Scheduler diagnostics and deterministic logging utilities.

use alloc::vec::Vec;

use super::execution_batch::ExecutionBatch;

/// Kinds of deterministic log entries emitted by the scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeterministicEvent {
  /// Timer registration event.
  Scheduled {
    /// Identifier of the registered handle.
    handle_id:     u64,
    /// Tick when the registration occurred.
    scheduled_tick: u64,
    /// Deadline tick assigned to the timer.
    deadline_tick:  u64,
  },
  /// Timer execution event.
  Fired {
    /// Identifier of the handle that executed.
    handle_id: u64,
    /// Tick when execution happened.
    fired_tick: u64,
    /// Execution metadata shared with the runnable/message.
    batch:      ExecutionBatch,
  },
  /// Timer cancellation event.
  Cancelled {
    /// Identifier of the cancelled handle.
    handle_id:     u64,
    /// Tick when the cancellation occurred.
    cancelled_tick: u64,
  },
}

/// Aggregates scheduler diagnostics state.
#[derive(Default)]
pub struct SchedulerDiagnostics {
  deterministic_log: Option<DeterministicLog>,
}

impl SchedulerDiagnostics {
  /// Creates a diagnostics container with logging disabled.
  #[must_use]
  pub const fn new() -> Self {
    Self { deterministic_log: None }
  }

  /// Enables deterministic logging with the requested capacity.
  pub fn enable_deterministic_log(&mut self, capacity: usize) {
    self.deterministic_log = Some(DeterministicLog::with_capacity(capacity));
  }

  /// Returns whether deterministic logging is enabled.
  #[must_use]
  pub const fn is_log_enabled(&self) -> bool {
    self.deterministic_log.is_some()
  }

  /// Returns the current log entries.
  #[must_use]
  pub fn deterministic_log(&self) -> &[DeterministicEvent] {
    self
      .deterministic_log
      .as_ref()
      .map_or(&[], |log| log.entries.as_slice())
  }

  pub(crate) fn record(&mut self, event: DeterministicEvent) {
    if let Some(log) = &mut self.deterministic_log {
      log.record(event);
    }
  }
}

struct DeterministicLog {
  entries:   Vec<DeterministicEvent>,
  capacity:  usize,
}

impl DeterministicLog {
  fn with_capacity(capacity: usize) -> Self {
    Self { entries: Vec::with_capacity(capacity), capacity }
  }

  fn record(&mut self, event: DeterministicEvent) {
    if self.entries.len() < self.capacity {
      self.entries.push(event);
    }
  }
}

impl Clone for SchedulerDiagnostics {
  fn clone(&self) -> Self {
    Self { deterministic_log: self.deterministic_log.clone() }
  }
}

impl Clone for DeterministicLog {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), capacity: self.capacity }
  }
}
