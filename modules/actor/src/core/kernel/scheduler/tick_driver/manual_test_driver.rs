//! Manual tick driver for deterministic testing (test-only).

#![cfg(any(test, feature = "test-support"))]

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::manual_tick_controller::ManualTickController;
use crate::core::kernel::scheduler::{
  SchedulerRunnerOwned, SchedulerShared, tick_driver::TickDriverProvisioningContext,
};

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

  pub(super) fn with_runner<F, R>(&self, mut f: F) -> Option<R>
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
