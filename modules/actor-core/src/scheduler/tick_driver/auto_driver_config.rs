//! Configuration for automatic tick driver selection.

use core::time::Duration;

use super::{FallbackPolicy, TickDriverAutoLocatorRef, TickDriverFactoryRef, TickMetricsMode};
use crate::RuntimeToolbox;

/// Configuration for automatic tick driver selection and behavior.
pub struct AutoDriverConfig<TB: RuntimeToolbox> {
  factory:    Option<TickDriverFactoryRef<TB>>,
  locator:    Option<TickDriverAutoLocatorRef<TB>>,
  resolution: Option<Duration>,
  metrics:    TickMetricsMode,
  fallback:   FallbackPolicy,
}

impl<TB: RuntimeToolbox> AutoDriverConfig<TB> {
  /// Creates a new auto driver configuration with default settings.
  #[must_use]
  pub fn new() -> Self {
    Self {
      factory:    None,
      locator:    None,
      resolution: None,
      metrics:    TickMetricsMode::default(),
      fallback:   FallbackPolicy::default(),
    }
  }

  /// Sets a specific driver factory, bypassing auto-detection.
  #[must_use]
  pub fn with_factory(mut self, factory: TickDriverFactoryRef<TB>) -> Self {
    self.factory = Some(factory);
    self
  }

  /// Sets a custom auto-locator.
  #[must_use]
  pub fn with_locator(mut self, locator: TickDriverAutoLocatorRef<TB>) -> Self {
    self.locator = Some(locator);
    self
  }

  /// Sets the tick resolution.
  #[must_use]
  pub fn with_resolution(mut self, resolution: Duration) -> Self {
    self.resolution = Some(resolution);
    self
  }

  /// Sets the metrics collection mode.
  #[must_use]
  pub fn with_metrics_mode(mut self, mode: TickMetricsMode) -> Self {
    self.metrics = mode;
    self
  }

  /// Sets the fallback policy for driver failures.
  #[must_use]
  pub fn with_fallback(mut self, policy: FallbackPolicy) -> Self {
    self.fallback = policy;
    self
  }

  /// Consumes and returns self (for method chaining convenience).
  #[must_use]
  pub const fn into_inner(self) -> Self {
    self
  }

  /// Returns the configured factory, if any.
  #[must_use]
  pub fn factory(&self) -> Option<&TickDriverFactoryRef<TB>> {
    self.factory.as_ref()
  }

  /// Returns the auto-locator, if configured.
  #[must_use]
  pub fn locator(&self) -> Option<&TickDriverAutoLocatorRef<TB>> {
    self.locator.as_ref()
  }

  /// Returns the configured resolution, if any.
  #[must_use]
  pub const fn resolution(&self) -> Option<Duration> {
    self.resolution
  }

  /// Returns the metrics mode.
  #[must_use]
  pub const fn metrics_mode(&self) -> &TickMetricsMode {
    &self.metrics
  }

  /// Returns the fallback policy.
  #[must_use]
  pub const fn fallback_policy(&self) -> &FallbackPolicy {
    &self.fallback
  }
}

impl<TB: RuntimeToolbox> Default for AutoDriverConfig<TB> {
  fn default() -> Self {
    Self::new()
  }
}
