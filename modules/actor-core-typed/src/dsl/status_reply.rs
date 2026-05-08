//! Status-aware reply type for typed ask patterns.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::dsl::StatusReplyError;

/// A reply that carries either a success value or a structured error.
///
/// This mirrors Pekko's `StatusReply[T]` and is intended for use with
/// [`ask_with_status`](crate::TypedActorRef::ask_with_status).
#[derive(Clone, Debug)]
pub enum StatusReply<T> {
  /// The operation succeeded with the given value.
  Success(T),
  /// The operation failed with the given error.
  Error(StatusReplyError),
}

impl<T> StatusReply<T> {
  /// Creates a success reply wrapping the given value.
  #[must_use]
  pub const fn success(value: T) -> Self {
    Self::Success(value)
  }

  /// Creates an error reply with the given message.
  #[must_use]
  pub fn error(message: impl Into<String>) -> Self {
    Self::Error(StatusReplyError::new(message))
  }

  /// Creates a success reply containing `()`, analogous to Pekko's `StatusReply.ack()`.
  #[must_use]
  pub const fn ack() -> StatusReply<()> {
    StatusReply::Success(())
  }

  /// Returns `true` if this is a success reply.
  #[must_use]
  pub const fn is_success(&self) -> bool {
    matches!(self, Self::Success(_))
  }

  /// Returns `true` if this is an error reply.
  #[must_use]
  pub const fn is_error(&self) -> bool {
    matches!(self, Self::Error(_))
  }

  /// Converts this status reply into a `Result`.
  ///
  /// # Errors
  ///
  /// Returns [`StatusReplyError`] when the reply is an error variant.
  pub fn into_result(self) -> Result<T, StatusReplyError> {
    match self {
      | Self::Success(value) => Ok(value),
      | Self::Error(err) => Err(err),
    }
  }
}
