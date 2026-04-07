//! Owned runner that encapsulates tick handle lifetime.

use fraktor_utils_core_rs::core::time::SchedulerTickHandle;

use super::{Scheduler, SchedulerRunner, tick_driver::SchedulerTickHandleOwned};

/// Owns a [`SchedulerRunner`] together with its tick handle.
pub struct SchedulerRunnerOwned {
  runner: SchedulerRunner<'static>,
  handle: SchedulerTickHandleOwned,
}

impl SchedulerRunnerOwned {
  /// Creates a new runner backed by an internal tick handle.
  #[must_use]
  pub fn new() -> Self {
    let handle = SchedulerTickHandleOwned::new();
    // SAFETY: handle owns the tick state for the lifetime of Self, therefore the
    // reference obtained here remains valid for `'static` as long as `self`
    // lives.
    let handle_ptr = handle.handle() as *const SchedulerTickHandle<'static>;
    let runner = unsafe { SchedulerRunner::new_internal(&*handle_ptr) };
    Self { runner, handle }
  }

  /// Injects ticks into the shared handle.
  pub fn inject(&self, ticks: u32) {
    self.handle.handle().inject_manual_ticks(ticks);
  }

  /// Drives the underlying scheduler for all pending ticks.
  pub fn drive(&mut self, scheduler: &mut Scheduler) {
    self.runner.run_once(scheduler);
  }

  /// Returns the owned tick handle.
  #[must_use]
  pub const fn handle(&self) -> &SchedulerTickHandle<'static> {
    self.handle.handle()
  }
}

impl Default for SchedulerRunnerOwned {
  fn default() -> Self {
    Self::new()
  }
}
