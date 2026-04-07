//! Tick driver configuration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::RuntimeMutex;

#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use super::{TickDriver, TickExecutorPump};

/// Configuration for tick driver creation.
pub enum TickDriverConfig {
  /// Runtime-wired configuration built from a driver and executor pump.
  Runtime {
    /// Driver source that publishes ticks into the core feed.
    driver:        RuntimeMutex<Box<dyn TickDriver>>,
    /// Runtime pump that drives the core scheduler executor.
    executor_pump: RuntimeMutex<Box<dyn TickExecutorPump>>,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver),
}

impl TickDriverConfig {
  /// Creates a runtime-wired tick driver configuration.
  #[must_use]
  pub fn runtime(driver: Box<dyn TickDriver>, executor_pump: Box<dyn TickExecutorPump>) -> Self {
    Self::Runtime { driver: RuntimeMutex::new(driver), executor_pump: RuntimeMutex::new(executor_pump) }
  }

  /// Creates a manual test driver configuration.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual(driver: ManualTestDriver) -> Self {
    Self::ManualTest(driver)
  }
}

impl core::fmt::Debug for TickDriverConfig {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::Runtime { .. } => f.debug_struct("Runtime").finish_non_exhaustive(),
      #[cfg(any(test, feature = "test-support"))]
      | Self::ManualTest(_) => f.debug_tuple("ManualTest").finish(),
    }
  }
}
