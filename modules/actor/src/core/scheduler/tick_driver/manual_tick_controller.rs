//! Public controller exposed to tests for manual tick injection.

#![cfg(any(test, feature = "test-support"))]

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

use super::manual_test_driver::ManualDriverState;

/// Public controller exposed to tests for manual tick injection.
pub struct ManualTickController {
  pub(super) state: ArcShared<ManualDriverState>,
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
