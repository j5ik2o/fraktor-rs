//! Tick driver configuration.

#[cfg(test)]
mod tests;

use core::time::Duration;

use super::{
  AutoDriverConfig, FallbackPolicy, TickDriverAutoLocatorRef, TickDriverFactoryRef, TickMetricsMode, TickPulseSource,
};
#[cfg(any(test, feature = "test-support"))]
use super::ManualTestDriver;
use crate::RuntimeToolbox;

/// Configuration for tick driver selection and behavior.
pub enum TickDriverConfig<TB: RuntimeToolbox> {
  /// Automatic driver selection based on runtime environment.
  Auto(AutoDriverConfig<TB>),
  /// Hardware timer driver with static pulse source.
  Hardware {
    /// Reference to hardware timer implementation.
    driver: &'static dyn TickPulseSource,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest(ManualTestDriver<TB>),
}

impl<TB: RuntimeToolbox> TickDriverConfig<TB> {
  /// Creates an automatic driver configuration with default settings.
  #[must_use]
  pub fn auto() -> Self {
    Self::Auto(AutoDriverConfig::new())
  }

  /// Creates an automatic driver configuration with explicit factory.
  #[must_use]
  pub fn auto_with_factory(factory: TickDriverFactoryRef<TB>) -> Self {
    Self::Auto(AutoDriverConfig::new().with_factory(factory))
  }

  /// Creates an automatic driver configuration with custom locator.
  #[must_use]
  pub fn auto_with_locator(locator: TickDriverAutoLocatorRef<TB>) -> Self {
    Self::Auto(AutoDriverConfig::new().with_locator(locator))
  }

  /// Creates a hardware driver configuration.
  #[must_use]
  pub const fn hardware(driver: &'static dyn TickPulseSource) -> Self {
    Self::Hardware { driver }
  }

  /// Creates a manual test driver configuration.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub fn manual(driver: ManualTestDriver<TB>) -> Self {
    Self::ManualTest(driver)
  }

  /// Sets fallback policy (only applies to Auto variant).
  #[must_use]
  pub fn with_fallback(self, policy: FallbackPolicy) -> Self {
    match self {
      | Self::Auto(cfg) => Self::Auto(cfg.with_fallback(policy)),
      | other => other,
    }
  }

  /// Sets metrics mode (only applies to Auto variant).
  #[must_use]
  pub fn with_metrics_mode(self, mode: TickMetricsMode) -> Self {
    match self {
      | Self::Auto(cfg) => Self::Auto(cfg.with_metrics_mode(mode)),
      | other => other,
    }
  }

  /// Sets tick resolution (only applies to Auto variant).
  #[must_use]
  pub fn with_resolution(self, resolution: Duration) -> Self {
    match self {
      | Self::Auto(cfg) => Self::Auto(cfg.with_resolution(resolution)),
      | other => other,
    }
  }
}
