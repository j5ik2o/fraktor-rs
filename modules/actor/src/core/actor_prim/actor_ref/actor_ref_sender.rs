//! Trait implemented by actor reference senders.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use super::send_outcome::SendOutcome;
use crate::core::{error::SendError, messaging::AnyMessageGeneric};

/// Abstraction over mailbox-backed senders.
pub trait ActorRefSender<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync {
  /// Sends a message to the underlying actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>>;

  /// Applies the delivery outcome after a successful send.
  ///
  /// Default behavior: execute any scheduled task; otherwise do nothing.
  fn apply_outcome(&mut self, outcome: SendOutcome) {
    match outcome {
      | SendOutcome::Delivered => {},
      | SendOutcome::Schedule(task) => task(),
    }
  }
}
