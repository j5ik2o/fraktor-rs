//! Tick driver configuration.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use super::{TickDriverBundle, TickDriverError};
use crate::core::scheduler::tick_driver::TickDriverProvisioningContext;

/// Type alias for tick driver builder function.
type TickDriverBuilderFn<TB> =
  Box<dyn Fn(&TickDriverProvisioningContext<TB>) -> Result<TickDriverBundle<TB>, TickDriverError> + Send + Sync>;

/// Configuration for tick driver creation.
pub enum TickDriverConfig<TB: RuntimeToolbox> {
  /// Builder function-based configuration (standard approach).
  Builder {
    /// Builder function that creates a complete tick driver bundle.
    builder: TickDriverBuilderFn<TB>,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver<TB>),
}

impl<TB: RuntimeToolbox> TickDriverConfig<TB> {
  /// Creates a tick driver configuration with a user-provided builder function.
  ///
  /// The builder function receives the provisioning context and must return a complete
  /// `TickDriverBundle` that includes both the tick driver and scheduler executor.
  #[must_use]
  pub fn new<F>(builder: F) -> Self
  where
    F: Fn(&TickDriverProvisioningContext<TB>) -> Result<TickDriverBundle<TB>, TickDriverError> + Send + Sync + 'static,
  {
    Self::Builder { builder: Box::new(builder) }
  }

  /// Creates a manual test driver configuration.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual(driver: ManualTestDriver<TB>) -> Self {
    Self::ManualTest(driver)
  }
}

impl<TB: RuntimeToolbox> core::fmt::Debug for TickDriverConfig<TB> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::Builder { .. } => f.debug_struct("Builder").finish_non_exhaustive(),
      #[cfg(any(test, feature = "test-support"))]
      | Self::ManualTest(_) => f.debug_tuple("ManualTest").finish(),
    }
  }
}
