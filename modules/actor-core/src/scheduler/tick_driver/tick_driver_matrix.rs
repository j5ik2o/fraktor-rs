//! Public driver matrix for quickstart guidance.

use core::time::Duration;

use super::{TickDriverKind, TickMetricsMode};

/// Entry describing a supported tick driver profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickDriverGuideEntry {
  /// Driver classification kind.
  pub kind:               TickDriverKind,
  /// Short label used in documentation tables.
  pub label:              &'static str,
  /// Human-readable description for guides.
  pub description:        &'static str,
  /// Default resolution exposed by the helper APIs.
  pub default_resolution: Duration,
  /// Metrics publishing mode configured for the driver.
  pub metrics_mode:       TickMetricsMode,
  /// Indicates whether the driver is limited to test builds.
  pub test_only:          bool,
}

impl TickDriverGuideEntry {
  /// Creates a new driver guide entry.
  #[must_use]
  pub const fn new(
    kind: TickDriverKind,
    label: &'static str,
    description: &'static str,
    default_resolution: Duration,
    metrics_mode: TickMetricsMode,
    test_only: bool,
  ) -> Self {
    Self { kind, label, description, default_resolution, metrics_mode, test_only }
  }

  /// Returns the auto-driver quickstart entry.
  #[must_use]
  pub const fn auto() -> Self {
    Self::new(
      TickDriverKind::Auto,
      "auto-std",
      "Tokio locator (StdTickDriverConfig::tokio_quickstart)",
      Duration::from_millis(10),
      TickMetricsMode::AutoPublish { interval: Duration::from_secs(1) },
      false,
    )
  }

  /// Returns the hardware driver quickstart entry.
  #[must_use]
  pub const fn hardware() -> Self {
    Self::new(
      TickDriverKind::Hardware { source: super::HardwareKind::Custom },
      "hardware",
      "TickPulseSource attachment for no_std targets",
      Duration::from_millis(1),
      TickMetricsMode::AutoPublish { interval: Duration::from_secs(1) },
      false,
    )
  }

  /// Returns the manual driver quickstart entry.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual() -> Self {
    Self::new(
      TickDriverKind::ManualTest,
      "manual-test",
      "Runner API (ManualTestDriver) for deterministic tests",
      Duration::from_millis(10),
      TickMetricsMode::OnDemand,
      true,
    )
  }
}

/// Driver selection matrix exposed to documentation and diagnostics helpers.
#[cfg(any(test, feature = "test-support"))]
pub const TICK_DRIVER_MATRIX: &[TickDriverGuideEntry] =
  &[TickDriverGuideEntry::auto(), TickDriverGuideEntry::hardware(), TickDriverGuideEntry::manual()];

/// Driver selection matrix exposed to documentation and diagnostics helpers (no manual entry).
#[cfg(not(any(test, feature = "test-support")))]
pub const TICK_DRIVER_MATRIX: &[TickDriverGuideEntry] =
  &[TickDriverGuideEntry::auto(), TickDriverGuideEntry::hardware()];
