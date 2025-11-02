//! Trait implemented by actor reference senders.

use crate::{RuntimeToolbox, any_message::AnyMessage, send_error::SendError};

/// Abstraction over mailbox-backed senders.
pub trait ActorRefSender<TB: RuntimeToolbox>: Send + Sync {
  /// Sends a message to the underlying actor.
  fn send(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>>;
}

#[cfg(test)]
mod tests;
