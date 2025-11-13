//! Tick-driven runner that advances the scheduler when new ticks arrive.

use core::marker::PhantomData;

use fraktor_utils_core_rs::time::{SchedulerTickHandle, TickLease};

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
  tick_handle: &'a SchedulerTickHandle<'a>,
  tick_lease:  TickLease<'a>,
  mode:        RunnerMode,
  _marker:     PhantomData<TB>,
}

impl<'a, TB: RuntimeToolbox + 'static> SchedulerRunner<'a, TB> {
  /// Creates a manual runner suitable for deterministic tests.
  #[must_use]
  pub fn manual(tick_handle: &'a SchedulerTickHandle<'a>) -> Self {
    let tick_lease = tick_handle.lease();
    Self { tick_handle, tick_lease, mode: RunnerMode::Manual, _marker: PhantomData }
  }

  /// Returns the configured mode.
  #[must_use]
  pub const fn mode(&self) -> RunnerMode {
    self.mode
  }

  /// Injects manual ticks that will be processed on the next [`run_once`] call.
  pub fn inject_manual_ticks(&self, ticks: u32) {
    self.tick_handle.inject_manual_ticks(ticks);
  }

  /// Processes the currently pending ticks.
  pub fn run_once(&mut self, scheduler: &mut Scheduler<TB>) {
    while let Some(event) = self.tick_lease.try_pull() {
      let ticks = event.ticks();
      if ticks == 0 {
        continue;
      }
      scheduler.run_for_ticks(u64::from(ticks));
    }
  }
}
