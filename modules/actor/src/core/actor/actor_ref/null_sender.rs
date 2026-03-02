//! Sender that rejects all messages.

#[cfg(test)]
mod tests;

use crate::core::{
  actor::actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

/// Sender that always returns a closed error.
#[derive(Default)]
pub struct NullSender;

impl ActorRefSender for NullSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}
