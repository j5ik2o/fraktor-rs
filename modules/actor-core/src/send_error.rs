//! Errors returned when enqueueing messages fails.

#[cfg(test)]
mod tests;

use alloc::fmt;

use crate::{AnyMessage, RuntimeToolbox};

/// Represents failures that can occur when enqueueing a message.
pub enum SendError<TB: RuntimeToolbox> {
  /// The mailbox is full and the message could not be enqueued.
  Full(AnyMessage<TB>),
  /// The mailbox is temporarily suspended.
  Suspended(AnyMessage<TB>),
  /// The mailbox or actor has been permanently closed.
  Closed(AnyMessage<TB>),
  /// No reply target was provided for the attempted send operation.
  NoRecipient(AnyMessage<TB>),
}

impl<TB: RuntimeToolbox> SendError<TB> {
  /// Creates a send error representing a full mailbox.
  #[must_use]
  pub const fn full(message: AnyMessage<TB>) -> Self {
    Self::Full(message)
  }

  /// Creates a send error representing a suspended mailbox.
  #[must_use]
  pub const fn suspended(message: AnyMessage<TB>) -> Self {
    Self::Suspended(message)
  }

  /// Creates a send error representing a closed mailbox or actor.
  #[must_use]
  pub const fn closed(message: AnyMessage<TB>) -> Self {
    Self::Closed(message)
  }

  /// Creates a send error representing a missing reply target.
  #[must_use]
  pub const fn no_recipient(message: AnyMessage<TB>) -> Self {
    Self::NoRecipient(message)
  }

  /// Returns a shared reference to the owned message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage<TB> {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message) => message,
    }
  }

  /// Consumes the error and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> AnyMessage<TB> {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message) => message,
    }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for SendError<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | SendError::Full(_) => f.debug_tuple("Full").finish(),
      | SendError::Suspended(_) => f.debug_tuple("Suspended").finish(),
      | SendError::Closed(_) => f.debug_tuple("Closed").finish(),
      | SendError::NoRecipient(_) => f.debug_tuple("NoRecipient").finish(),
    }
  }
}
