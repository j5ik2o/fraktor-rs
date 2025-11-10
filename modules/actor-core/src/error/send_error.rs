//! Errors returned when enqueueing messages fails.

#[cfg(test)]
mod tests;

use alloc::fmt;

use crate::{NoStdToolbox, RuntimeToolbox, messaging::AnyMessageGeneric};

/// Represents failures that can occur when enqueueing a message.
pub enum SendError<TB: RuntimeToolbox = NoStdToolbox> {
  /// The mailbox is full and the message could not be enqueued.
  Full(AnyMessageGeneric<TB>),
  /// The mailbox is temporarily suspended.
  Suspended(AnyMessageGeneric<TB>),
  /// The mailbox or actor has been permanently closed.
  Closed(AnyMessageGeneric<TB>),
  /// No reply target was provided for the attempted send operation.
  NoRecipient(AnyMessageGeneric<TB>),
  /// The mailbox failed to accept the message before the timeout elapsed.
  Timeout(AnyMessageGeneric<TB>),
}

impl<TB: RuntimeToolbox> SendError<TB> {
  /// Creates a send error representing a full mailbox.
  #[must_use]
  pub const fn full(message: AnyMessageGeneric<TB>) -> Self {
    Self::Full(message)
  }

  /// Creates a send error representing a suspended mailbox.
  #[must_use]
  pub const fn suspended(message: AnyMessageGeneric<TB>) -> Self {
    Self::Suspended(message)
  }

  /// Creates a send error representing a closed mailbox or actor.
  #[must_use]
  pub const fn closed(message: AnyMessageGeneric<TB>) -> Self {
    Self::Closed(message)
  }

  /// Creates a send error representing a missing reply target.
  #[must_use]
  pub const fn no_recipient(message: AnyMessageGeneric<TB>) -> Self {
    Self::NoRecipient(message)
  }

  /// Creates a send error representing an enqueue timeout.
  #[must_use]
  pub const fn timeout(message: AnyMessageGeneric<TB>) -> Self {
    Self::Timeout(message)
  }

  /// Returns a shared reference to the owned message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessageGeneric<TB> {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message)
      | SendError::Timeout(message) => message,
    }
  }

  /// Consumes the error and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> AnyMessageGeneric<TB> {
    match self {
      | SendError::Full(message)
      | SendError::Suspended(message)
      | SendError::Closed(message)
      | SendError::NoRecipient(message)
      | SendError::Timeout(message) => message,
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
      | SendError::Timeout(_) => f.debug_tuple("Timeout").finish(),
    }
  }
}
