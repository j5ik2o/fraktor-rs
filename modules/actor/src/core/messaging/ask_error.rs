//! Errors returned by ask operations.

#[cfg(test)]
mod tests;

/// Represents failures that can occur during an ask operation.
///
/// This error type is used as the `Err` variant in `AskResult` to distinguish
/// between successful replies and various failure conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskError {
  /// The ask operation timed out before receiving a reply.
  Timeout,
  /// The target actor could not be found or the message was undeliverable.
  DeadLetter,
  /// The message could not be sent to the target actor.
  SendFailed,
}

impl core::fmt::Display for AskError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | AskError::Timeout => f.write_str("AskError::Timeout"),
      | AskError::DeadLetter => f.write_str("AskError::DeadLetter"),
      | AskError::SendFailed => f.write_str("AskError::SendFailed"),
    }
  }
}
