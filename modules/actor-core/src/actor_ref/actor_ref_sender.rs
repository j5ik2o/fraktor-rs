//! Trait for actor message sending.

use crate::{any_message::AnyOwnedMessage, send_error::SendError};

/// Trait implemented by mailbox endpoints that accept [`AnyOwnedMessage`] instances.
pub trait ActorRefSender: Send + Sync {
  /// Enqueues the message into the underlying mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full, closed, or otherwise unable to accept the message.
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError>;
}
