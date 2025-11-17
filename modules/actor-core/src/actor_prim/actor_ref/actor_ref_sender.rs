//! Trait implemented by actor reference senders.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::NoStdToolbox;

use crate::{RuntimeToolbox, error::SendError, messaging::AnyMessageGeneric};

/// Abstraction over mailbox-backed senders.
pub trait ActorRefSender<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync {
  /// Sends a message to the underlying actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>>;
}
