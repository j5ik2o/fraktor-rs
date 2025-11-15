//! Tick driver variant classification.

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

/// Hardware timer source classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HardwareKind {
  /// Embassy-based timer.
  Embassy,
  /// SysTick-based timer.
  SysTick,
  /// Custom hardware timer.
  Custom,
}
