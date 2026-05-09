//! Errors returned when enqueueing messages fails.

#[cfg(test)]
mod tests;

use alloc::fmt::{Debug, Formatter, Result as FmtResult};

use crate::actor::messaging::AnyMessage;

/// Represents failures that can occur when enqueueing a message.
pub enum SendError {
  /// The mailbox is full and the message could not be enqueued.
  Full(AnyMessage),
  /// The mailbox is temporarily suspended.
  Suspended(AnyMessage),
  /// The mailbox or actor has been permanently closed.
  Closed(AnyMessage),
  /// No reply target was provided for the attempted send operation.
  NoRecipient(AnyMessage),
  /// The mailbox failed to accept the message before the timeout elapsed.
  Timeout(AnyMessage),
  /// The sender rejected the payload because its runtime type was not accepted.
  InvalidPayload {
    /// The original message that failed payload validation.
    message: AnyMessage,
    /// Short description of the expected payload contract.
    context: &'static str,
  },
}

impl SendError {
  /// Creates a send error representing a full mailbox.
  #[must_use]
  pub const fn full(message: AnyMessage) -> Self {
    Self::Full(message)
  }

  /// Creates a send error representing a suspended mailbox.
  #[must_use]
  pub const fn suspended(message: AnyMessage) -> Self {
    Self::Suspended(message)
  }

  /// Creates a send error representing a closed mailbox or actor.
  #[must_use]
  pub const fn closed(message: AnyMessage) -> Self {
    Self::Closed(message)
  }

  /// Creates a send error representing a missing reply target.
  #[must_use]
  pub const fn no_recipient(message: AnyMessage) -> Self {
    Self::NoRecipient(message)
  }

  /// Creates a send error representing an enqueue timeout.
  #[must_use]
  pub const fn timeout(message: AnyMessage) -> Self {
    Self::Timeout(message)
  }

  /// Creates a send error representing a payload type mismatch.
  #[must_use]
  pub const fn invalid_payload(message: AnyMessage, context: &'static str) -> Self {
    Self::InvalidPayload { message, context }
  }

  /// Returns a shared reference to the owned message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message)
      | SendError::Timeout(message) => message,
      | SendError::InvalidPayload { message, .. } => message,
    }
  }

  /// Consumes the error and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> AnyMessage {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message)
      | SendError::Timeout(message) => message,
      | SendError::InvalidPayload { message, .. } => message,
    }
  }
}

impl Debug for SendError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | SendError::Full(_) => f.debug_tuple("Full").finish(),
      | SendError::Suspended(_) => f.debug_tuple("Suspended").finish(),
      | SendError::Closed(_) => f.debug_tuple("Closed").finish(),
      | SendError::NoRecipient(_) => f.debug_tuple("NoRecipient").finish(),
      | SendError::Timeout(_) => f.debug_tuple("Timeout").finish(),
      | SendError::InvalidPayload { context, .. } => f.debug_tuple("InvalidPayload").field(context).finish(),
    }
  }
}
