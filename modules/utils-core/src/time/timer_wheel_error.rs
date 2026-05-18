//! Errors returned by the timer wheel.

use core::fmt::{self, Formatter, Result as FmtResult};

/// Errors emitted by the timer wheel.
#[derive(Debug, PartialEq, Eq)]
pub enum TimerWheelError {
  /// Resolution mismatch between configuration and entry.
  ResolutionMismatch,
  /// Wheel reached the configured capacity.
  CapacityExceeded,
}

impl fmt::Display for TimerWheelError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::ResolutionMismatch => f.write_str("timer entry resolution mismatch"),
      | Self::CapacityExceeded => f.write_str("timer wheel capacity exceeded"),
    }
  }
}

impl core::error::Error for TimerWheelError {}
