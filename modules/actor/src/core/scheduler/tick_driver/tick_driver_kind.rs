//! Tick driver variant classification.

use super::HardwareKind;

/// Classification of tick driver implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickDriverKind {
  /// Automatic driver selected by runtime detection.
  Auto,
  /// Hardware timer driver for embedded targets.
  Hardware {
    /// Hardware timer source type.
    source: HardwareKind,
  },
  /// Manual test driver (test-only).
  #[cfg(any(test, feature = "test-support"))]
  ManualTest,
}
