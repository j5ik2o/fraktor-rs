//! Classification of hardware timer sources.

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
