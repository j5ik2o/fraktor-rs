//! Tick-driven runner that advances the scheduler when new ticks arrive.

use super::Scheduler;
use crate::RuntimeToolbox;

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
pub struct SchedulerRunner<'a, TB: RuntimeToolbox + 'static> {
  scheduler:     &'a mut Scheduler<TB>,
  pending_ticks: u32,
  mode:          RunnerMode,
}

impl<'a, TB: RuntimeToolbox + 'static> SchedulerRunner<'a, TB> {
  /// Creates a manual runner suitable for deterministic tests.
  #[must_use]
  pub fn manual(scheduler: &'a mut Scheduler<TB>) -> Self {
    Self { scheduler, pending_ticks: 0, mode: RunnerMode::Manual }
  }

  /// Returns the configured mode.
  #[must_use]
  pub const fn mode(&self) -> RunnerMode {
    self.mode
  }

  /// Injects manual ticks that will be processed on the next [`run_once`] call.
  pub fn inject_manual_ticks(&mut self, ticks: u32) {
    self.pending_ticks = self.pending_ticks.saturating_add(ticks);
  }

  /// Processes the currently pending ticks.
  pub fn run_once(&mut self) {
    if self.pending_ticks == 0 {
      return;
    }
    let ticks = self.pending_ticks;
    self.pending_ticks = 0;
    self.scheduler.run_for_ticks(u64::from(ticks));
  }
}
