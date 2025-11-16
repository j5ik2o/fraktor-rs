//! Tick driver configuration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use super::{TickDriverError, TickDriverRuntime};
use crate::{RuntimeToolbox, scheduler::SchedulerContext};

/// Configuration for tick driver creation.
pub enum TickDriverConfig<TB: RuntimeToolbox> {
  /// Builder function-based configuration (standard approach).
  Builder {
    /// Builder function that creates a complete tick driver runtime.
    builder: Box<dyn Fn(&SchedulerContext<TB>) -> Result<TickDriverRuntime<TB>, TickDriverError> + Send + Sync>,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver<TB>),
}

impl<TB: RuntimeToolbox> TickDriverConfig<TB> {
  /// Creates a tick driver configuration with a user-provided builder function.
  ///
  /// The builder function receives the scheduler context and must return a complete
  /// `TickDriverRuntime` that includes both the tick driver and scheduler executor.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// TickDriverConfig::new(|ctx| {
  ///   // Create tick driver runtime...
  ///   Ok(runtime)
  /// })
  /// ```
  #[must_use]
  pub fn new<F>(builder: F) -> Self
  where
    F: Fn(&SchedulerContext<TB>) -> Result<TickDriverRuntime<TB>, TickDriverError> + Send + Sync + 'static, {
    Self::Builder { builder: Box::new(builder) }
  }

  /// Creates a manual test driver configuration.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual(driver: ManualTestDriver<TB>) -> Self {
    Self::ManualTest(driver)
  }
}
