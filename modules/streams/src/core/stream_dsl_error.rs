use core::fmt;

use super::OperatorKey;

/// Errors returned by stream DSL operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDslError {
  /// Indicates an invalid argument at stream construction.
  InvalidArgument {
    /// Invalid argument name.
    name:   &'static str,
    /// Invalid argument value.
    value:  usize,
    /// Human-readable failure reason.
    reason: &'static str,
  },
  /// Indicates an operator key outside of the compatibility catalog.
  UnsupportedOperator {
    /// Unsupported operator key.
    key: OperatorKey,
  },
}

impl fmt::Display for StreamDslError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::InvalidArgument { name, value, reason } => {
        write!(f, "invalid argument `{name}` ({value}): {reason}")
      },
      | Self::UnsupportedOperator { key } => write!(f, "unsupported operator `{}`", key.as_str()),
    }
  }
}
