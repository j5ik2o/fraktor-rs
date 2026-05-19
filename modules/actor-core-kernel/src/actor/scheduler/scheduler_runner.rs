//! Tick-driven runner that advances the scheduler when new ticks arrive.

// Issue #413: RunnerMode は SchedulerRunner のフィールド型としてのみ使用されるため同居させる。
#![allow(multiple_type_definitions)]

use core::marker::PhantomData;

use fraktor_utils_core_rs::time::{SchedulerTickHandle, TickLease};

use super::Scheduler;

/// Runner operating mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerMode {
  /// Manual driver using deterministic tick injection.
  Manual,
  /// Placeholder for async host drivers (tokio, std timers).
  AsyncHost,
  /// Placeholder for hardware-backed drivers (embassy/SysTick).
  Hardware,
}

/// Drives [`Scheduler`] by draining manually injected ticks.
pub struct SchedulerRunner<'a> {
  tick_handle: &'a SchedulerTickHandle<'a>,
  tick_lease:  TickLease<'a>,
  mode:        RunnerMode,
  _marker:     PhantomData<()>,
}

impl<'a> SchedulerRunner<'a> {
  #[must_use]
  pub(crate) fn new_internal(tick_handle: &'a SchedulerTickHandle<'a>) -> Self {
    let tick_lease = tick_handle.lease();
    Self { tick_handle, tick_lease, mode: RunnerMode::Manual, _marker: PhantomData }
  }

  /// Creates a manual runner suitable for deterministic tests.
  ///
  /// Inline-test only helper kept always-present (not test-cfg gated) so that test files
  /// across the crate can share it via `pub(crate)` visibility.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn manual(tick_handle: &'a SchedulerTickHandle<'a>) -> Self {
    Self::new_internal(tick_handle)
  }

  /// Returns the configured mode.
  #[must_use]
  pub const fn mode(&self) -> RunnerMode {
    self.mode
  }

  /// Injects manual ticks that will be processed on the next [`Self::run_once`] call.
  pub fn inject_manual_ticks(&self, ticks: u32) {
    self.tick_handle.inject_manual_ticks(ticks);
  }

  /// Processes the currently pending ticks.
  pub fn run_once(&mut self, scheduler: &mut Scheduler) {
    while let Some(ticks) = self.tick_lease.try_pull() {
      scheduler.run_for_ticks(u64::from(ticks));
    }
  }
}
