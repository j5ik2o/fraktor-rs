//! Tick driver variant classification.

// Issue #413: HardwareKind は TickDriverKind のフィールド型としてのみ使用されるため同居させる。
#![allow(multiple_type_definitions)]

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
