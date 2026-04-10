//! Tick driver configuration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::fmt::{Debug, Formatter, Result as FmtResult};

use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use super::{TickDriver, TickExecutorPump};

/// Configuration for tick driver creation.
pub enum TickDriverConfig {
  /// Runtime-wired configuration built from a driver and executor pump.
  Runtime {
    /// Driver source that publishes ticks into the core feed.
    driver:        SharedLock<Box<dyn TickDriver>>,
    /// Runtime pump that drives the core scheduler executor.
    executor_pump: SharedLock<Box<dyn TickExecutorPump>>,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver),
}

impl TickDriverConfig {
  /// Creates a runtime-wired tick driver configuration.
  #[must_use]
  pub fn runtime(driver: Box<dyn TickDriver>, executor_pump: Box<dyn TickExecutorPump>) -> Self {
    Self::Runtime {
      driver:        SharedLock::new_with_driver::<SpinSyncMutex<_>>(driver),
      executor_pump: SharedLock::new_with_driver::<SpinSyncMutex<_>>(executor_pump),
    }
  }

  /// Creates a manual test driver configuration.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual(driver: ManualTestDriver) -> Self {
    Self::ManualTest(driver)
  }
}

impl Debug for TickDriverConfig {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Runtime { .. } => f.debug_struct("Runtime").finish_non_exhaustive(),
      #[cfg(any(test, feature = "test-support"))]
      | Self::ManualTest(_) => f.debug_tuple("ManualTest").finish(),
    }
  }
}
