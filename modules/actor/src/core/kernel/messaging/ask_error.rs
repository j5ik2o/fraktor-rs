//! Errors returned by ask operations.

#[cfg(test)]
mod tests;

use alloc::format;

use crate::core::kernel::error::{ActorErrorReason, SendError};

/// Represents failures that can occur during an ask operation.
///
/// This error type is used as the `Err` variant in `AskResult` to distinguish
/// between successful replies and various failure conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskError {
  /// The ask operation timed out before receiving a reply.
  Timeout,
  /// The target actor could not be found or the message was undeliverable.
  DeadLetter,
  /// The message could not be sent to the target actor.
  SendFailed(ActorErrorReason),
}

impl AskError {
  /// Creates a send-failed error preserving the underlying failure context.
  #[must_use]
  pub fn send_failed(reason: impl Into<ActorErrorReason>) -> Self {
    Self::SendFailed(reason.into())
  }
}

impl From<&SendError> for AskError {
  fn from(error: &SendError) -> Self {
    Self::send_failed(ActorErrorReason::typed::<SendError>(format!("{error:?}")))
  }
}

impl core::fmt::Display for AskError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | AskError::Timeout => f.write_str("AskError::Timeout"),
      | AskError::DeadLetter => f.write_str("AskError::DeadLetter"),
      | AskError::SendFailed(reason) => write!(f, "AskError::SendFailed({reason})"),
    }
  }
}
