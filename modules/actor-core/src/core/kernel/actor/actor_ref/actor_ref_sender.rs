//! Trait implemented by actor reference senders.

#[cfg(test)]
mod tests;

use super::send_outcome::SendOutcome;
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

/// Abstraction over mailbox-backed senders.
pub trait ActorRefSender: Send + Sync {
  /// Sends a message to the underlying actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError>;

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
