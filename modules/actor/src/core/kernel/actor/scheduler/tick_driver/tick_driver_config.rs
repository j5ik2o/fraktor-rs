//! Tick driver configuration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use super::{TickDriverBundle, TickDriverError};
use crate::core::kernel::actor::scheduler::tick_driver::TickDriverProvisioningContext;
type TickDriverBuilderFn =
  Box<dyn Fn(&TickDriverProvisioningContext) -> Result<TickDriverBundle, TickDriverError> + Send + Sync>;

/// Configuration for tick driver creation.
pub enum TickDriverConfig {
  /// Builder function-based configuration (standard approach).
  Builder {
    /// Builder function that creates a complete tick driver bundle.
    builder: TickDriverBuilderFn,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver),
}

impl TickDriverConfig {
  /// Creates a tick driver configuration with a user-provided builder function.
  ///
  /// The builder function receives the provisioning context and must return a complete
  /// `TickDriverBundle` that includes both the tick driver and scheduler executor.
  #[must_use]
  pub fn new<F>(builder: F) -> Self
  where
    F: Fn(&TickDriverProvisioningContext) -> Result<TickDriverBundle, TickDriverError> + Send + Sync + 'static, {
    Self::Builder { builder: Box::new(builder) }
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
      | Self::Builder { .. } => f.debug_struct("Builder").finish_non_exhaustive(),
      #[cfg(any(test, feature = "test-support"))]
      | Self::ManualTest(_) => f.debug_tuple("ManualTest").finish(),
    }
  }
}
