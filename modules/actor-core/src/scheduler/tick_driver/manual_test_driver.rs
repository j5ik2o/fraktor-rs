//! Manual tick driver for deterministic testing (test-only).

#![cfg(any(test, feature = "test-support"))]

use fraktor_utils_core_rs::sync::{ArcShared, sync_mutex_like::{SpinSyncMutex, SyncMutexLike}};

use crate::{RuntimeToolbox, ToolboxMutex, scheduler::{Scheduler, SchedulerContext, SchedulerRunnerOwned}};

/// Manual tick driver for deterministic testing.
#[derive(Clone)]
pub struct ManualTestDriver<TB: RuntimeToolbox> {
  state: ArcShared<ManualDriverState<TB>>,
}

impl<TB: RuntimeToolbox> ManualTestDriver<TB> {
  /// Creates a new manual test driver.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(ManualDriverState::new()) }
  }

  /// Returns a controller that can inject ticks and drive the scheduler.
  #[must_use]
  pub fn controller(&self) -> ManualTickController<TB> {
    ManualTickController { state: self.state.clone() }
  }

  pub(crate) fn attach(&self, ctx: &SchedulerContext<TB>) {
    self.state.attach(ctx.scheduler());
  }

  pub(crate) fn state(&self) -> ArcShared<ManualDriverState<TB>> {
    self.state.clone()
  }
}

impl<TB: RuntimeToolbox> Default for ManualTestDriver<TB> {
  fn default() -> Self {
    Self::new()
  }
}

pub(crate) struct ManualDriverState<TB: RuntimeToolbox> {
  scheduler: SpinSyncMutex<Option<ArcShared<ToolboxMutex<Scheduler<TB>, TB>>>>,
  runner:    SpinSyncMutex<Option<SchedulerRunnerOwned<TB>>>,
}

impl<TB: RuntimeToolbox> ManualDriverState<TB> {
  const fn new() -> Self {
    Self { scheduler: SpinSyncMutex::new(None), runner: SpinSyncMutex::new(None) }
  }

  fn attach(&self, scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>) {
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
    F: FnMut(&mut SchedulerRunnerOwned<TB>, &ArcShared<ToolboxMutex<Scheduler<TB>, TB>>) -> R,
  {
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
pub struct ManualTickController<TB: RuntimeToolbox> {
  state: ArcShared<ManualDriverState<TB>>,
}

impl<TB: RuntimeToolbox> Clone for ManualTickController<TB> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<TB: RuntimeToolbox> ManualTickController<TB> {
  /// Injects ticks into the scheduler without running it.
  pub fn inject_ticks(&self, ticks: u32) {
    self.state.with_runner(|runner, _| {
      runner.inject(ticks);
    });
  }

  /// Drives the scheduler for pending ticks.
  pub fn drive(&self) {
    self.state.with_runner(|runner, scheduler| {
      let mut guard = scheduler.lock();
      runner.drive(&mut guard);
    });
  }

  /// Convenience helper that injects ticks and drives immediately.
  pub fn inject_and_drive(&self, ticks: u32) {
    self.inject_ticks(ticks);
    self.drive();
  }
}

/// Control hook that detaches manual driver state on shutdown.
pub(crate) struct ManualDriverControl<TB: RuntimeToolbox> {
  state: ArcShared<ManualDriverState<TB>>,
}

impl<TB: RuntimeToolbox> ManualDriverControl<TB> {
  pub(crate) fn new(state: ArcShared<ManualDriverState<TB>>) -> Self {
    Self { state }
  }
}

impl<TB: RuntimeToolbox> super::TickDriverControl for ManualDriverControl<TB> {
  fn shutdown(&self) {
    self.state.reset();
  }
}
