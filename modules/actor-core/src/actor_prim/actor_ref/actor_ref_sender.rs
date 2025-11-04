//! Trait implemented by actor reference senders.

use cellactor_utils_core_rs::sync::NoStdToolbox;

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
