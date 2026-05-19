//! Snapshot describing scheduler metrics and state.

use alloc::vec::Vec;
use core::time::Duration;

use super::{dump_job::SchedulerDumpJob, metrics::SchedulerMetrics, warning::SchedulerWarning};

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
  pub const fn new(
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
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn jobs(&self) -> &[SchedulerDumpJob] {
    &self.jobs
  }

  /// Returns warnings accumulated up to the dump.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn warnings(&self) -> &[SchedulerWarning] {
    &self.warnings
  }
}
