//! Trait implemented by actor reference senders.

use crate::{RuntimeToolbox, error::SendError, messaging::AnyMessage};

/// Abstraction over mailbox-backed senders.
pub trait ActorRefSender<TB: RuntimeToolbox>: Send + Sync {
  /// Sends a message to the underlying actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  fn send(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>>;
}
