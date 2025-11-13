//! Snapshot structures describing scheduler state.

use alloc::vec::Vec;
use core::time::Duration;

use super::{metrics::SchedulerMetrics, mode::SchedulerMode, warning::SchedulerWarning};

/// Snapshot exposing scheduler internals for diagnostics and CLI tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerDump {
  resolution:   Duration,
  current_tick: u64,
  metrics:      SchedulerMetrics,
  jobs:         Vec<SchedulerDumpJob>,
  warnings:     Vec<SchedulerWarning>,
}

impl SchedulerDump {
  /// Creates a dump from the provided components.
  #[must_use]
  pub fn new(
    resolution: Duration,
    current_tick: u64,
    metrics: SchedulerMetrics,
    jobs: Vec<SchedulerDumpJob>,
    warnings: Vec<SchedulerWarning>,
  ) -> Self {
    Self { resolution, current_tick, metrics, jobs, warnings }
  }

  /// Returns the configured tick resolution.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Logical tick captured when the dump was generated.
  #[must_use]
  pub const fn current_tick(&self) -> u64 {
    self.current_tick
  }

  /// Metrics snapshot associated with the dump.
  #[must_use]
  pub const fn metrics(&self) -> SchedulerMetrics {
    self.metrics
  }

  /// Returns pending jobs recorded in the dump.
  #[must_use]
  pub fn jobs(&self) -> &[SchedulerDumpJob] {
    &self.jobs
  }

  /// Returns warnings accumulated up to the dump.
  #[must_use]
  pub fn warnings(&self) -> &[SchedulerWarning] {
    &self.warnings
  }
}

/// Metadata describing a scheduled job inside the dump snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerDumpJob {
  /// Handle identifier for the job.
  pub handle_id: u64,
  /// Scheduling mode (one-shot, fixed-rate, fixed-delay).
  pub mode:      SchedulerMode,
  /// Deadline tick recorded when the dump was produced.
  pub deadline_tick: u64,
  /// Next periodic tick when available.
  pub next_tick:     Option<u64>,
}

impl SchedulerDumpJob {
  /// Creates a new dump job entry.
  #[must_use]
  pub const fn new(handle_id: u64, mode: SchedulerMode, deadline_tick: u64, next_tick: Option<u64>) -> Self {
    Self { handle_id, mode, deadline_tick, next_tick }
  }
}
