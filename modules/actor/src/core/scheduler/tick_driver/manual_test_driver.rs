//! Manual tick driver for deterministic testing (test-only).

#![cfg(any(test, feature = "test-support"))]

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess, sync_mutex_like::SpinSyncMutex};

use crate::core::scheduler::{SchedulerRunnerOwned, SchedulerShared, tick_driver::TickDriverProvisioningContext};

type SchedulerContextMutex = SpinSyncMutex<Option<SchedulerShared>>;

/// Manual tick driver for deterministic testing.
#[derive(Clone)]
pub struct ManualTestDriver {
  state: ArcShared<ManualDriverState>,
}

impl ManualTestDriver {
  /// Creates a new manual test driver.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(ManualDriverState::new()) }
  }

  /// Returns a controller that can inject ticks and drive the scheduler.
  #[must_use]
  pub fn controller(&self) -> ManualTickController {
    ManualTickController { state: self.state.clone() }
  }

  pub(crate) fn attach(&self, ctx: &TickDriverProvisioningContext) {
    self.state.attach(ctx.scheduler());
  }

  pub(crate) fn state(&self) -> ArcShared<ManualDriverState> {
    self.state.clone()
  }
}

impl Default for ManualTestDriver {
  fn default() -> Self {
    Self::new()
  }
}

pub(crate) struct ManualDriverState {
  scheduler: SchedulerContextMutex,
  runner:    SpinSyncMutex<Option<SchedulerRunnerOwned>>,
}

impl ManualDriverState {
  const fn new() -> Self {
    Self { scheduler: SpinSyncMutex::new(None), runner: SpinSyncMutex::new(None) }
  }

  fn attach(&self, scheduler: SchedulerShared) {
    *self.scheduler.lock() = Some(scheduler);
    let mut runner = self.runner.lock();
    if runner.is_none() {
      *runner = Some(SchedulerRunnerOwned::new());
    }
  }

  fn reset(&self) {
    *self.scheduler.lock() = None;
    *self.runner.lock() = None;
  }

  fn with_runner<F, R>(&self, mut f: F) -> Option<R>
  where
    F: FnMut(&mut SchedulerRunnerOwned, &SchedulerShared) -> R, {
    let scheduler = self.scheduler.lock().clone();
    let mut runner_guard = self.runner.lock();
    if let (Some(scheduler), Some(runner)) = (scheduler, runner_guard.as_mut()) {
      Some(f(runner, &scheduler))
    } else {
      None
    }
  }
}

/// Public controller exposed to tests for manual tick injection.
pub struct ManualTickController {
  state: ArcShared<ManualDriverState>,
}

impl Clone for ManualTickController {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl ManualTickController {
  /// Injects ticks into the scheduler without running it.
  pub fn inject_ticks(&self, ticks: u32) {
    self.state.with_runner(|runner, _| {
      runner.inject(ticks);
    });
  }

  /// Drives the scheduler for pending ticks.
  pub fn drive(&self) {
    self.state.with_runner(|runner, scheduler| {
      scheduler.with_write(|s| runner.drive(s));
    });
  }

  /// Convenience helper that injects ticks and drives immediately.
  pub fn inject_and_drive(&self, ticks: u32) {
    self.inject_ticks(ticks);
    self.drive();
  }
}

/// Control hook that detaches manual driver state on shutdown.
pub(crate) struct ManualDriverControl {
  state: ArcShared<ManualDriverState>,
}

impl ManualDriverControl {
  pub(crate) const fn new(state: ArcShared<ManualDriverState>) -> Self {
    Self { state }
  }
}

impl super::TickDriverControl for ManualDriverControl {
  fn shutdown(&self) {
    self.state.reset();
  }
}
