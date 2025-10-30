//! Errors produced when sending messages through an `ActorRef`.

use core::fmt;

use crate::any_message::AnyOwnedMessage;

/// Represents failures that can occur when enqueueing a message.
pub enum SendError {
  /// The mailbox is full and the message could not be enqueued.
  Full(AnyOwnedMessage),
  /// The mailbox is temporarily suspended.
  Suspended(AnyOwnedMessage),
  /// The mailbox or actor has been permanently closed.
  Closed(AnyOwnedMessage),
  /// No reply target was provided for the attempted send operation.
  NoRecipient(AnyOwnedMessage),
}

impl fmt::Debug for SendError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SendError::Full(_) => f.debug_tuple("Full").finish(),
      SendError::Suspended(_) => f.debug_tuple("Suspended").finish(),
      SendError::Closed(_) => f.debug_tuple("Closed").finish(),
      SendError::NoRecipient(_) => f.debug_tuple("NoRecipient").finish(),
    }
  }
}

impl SendError {
  /// Creates a send error representing a full mailbox.
  #[must_use]
  pub fn full(message: AnyOwnedMessage) -> Self {
    Self::Full(message)
  }

  /// Creates a send error representing a suspended mailbox.
  #[must_use]
  pub fn suspended(message: AnyOwnedMessage) -> Self {
    Self::Suspended(message)
  }

  /// Creates a send error representing a closed mailbox or actor.
  #[must_use]
  pub fn closed(message: AnyOwnedMessage) -> Self {
    Self::Closed(message)
  }

  /// Creates a send error representing a missing reply target.
  #[must_use]
  pub fn no_recipient(message: AnyOwnedMessage) -> Self {
    Self::NoRecipient(message)
  }

  /// Returns a shared reference to the owned message.
  #[must_use]
  pub fn message(&self) -> &AnyOwnedMessage {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message) => message,
    }
  }

  /// Consumes the error and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> AnyOwnedMessage {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message) => message,
    }
  }
}
