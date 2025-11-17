//! Tick-driven runner that advances the scheduler when new ticks arrive.

use core::marker::PhantomData;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  time::{SchedulerTickHandle, TickLease},
};

use super::{RunnerMode, Scheduler};

/// Drives [`Scheduler`] by draining manually injected ticks.
pub struct SchedulerRunner<'a, TB: RuntimeToolbox + 'static> {
  tick_handle: &'a SchedulerTickHandle<'a>,
  tick_lease:  TickLease<'a>,
  mode:        RunnerMode,
  _marker:     PhantomData<TB>,
}

impl<'a, TB: RuntimeToolbox + 'static> SchedulerRunner<'a, TB> {
  #[must_use]
  pub(crate) fn new_internal(tick_handle: &'a SchedulerTickHandle<'a>) -> Self {
    let tick_lease = tick_handle.lease();
    Self { tick_handle, tick_lease, mode: RunnerMode::Manual, _marker: PhantomData }
  }

  /// Creates a manual runner suitable for deterministic tests.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub fn manual(tick_handle: &'a SchedulerTickHandle<'a>) -> Self {
    Self::new_internal(tick_handle)
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
