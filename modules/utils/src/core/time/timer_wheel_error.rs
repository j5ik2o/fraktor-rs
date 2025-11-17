//! Errors returned by the timer wheel.

/// Errors emitted by the timer wheel.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TimerWheelError {
  /// Resolution mismatch between configuration and entry.
  #[error("timer entry resolution mismatch")]
  ResolutionMismatch,
  /// Wheel reached the configured capacity.
  #[error("timer wheel capacity exceeded")]
  CapacityExceeded,
}
