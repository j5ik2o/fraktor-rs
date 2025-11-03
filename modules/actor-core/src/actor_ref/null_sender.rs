//! Sender that rejects all messages.

#[cfg(test)]
mod tests;

use crate::{
  RuntimeToolbox, actor_ref::actor_ref_sender::ActorRefSender, any_message::AnyMessage, send_error::SendError,
};

/// Sender that always returns a closed error.
#[derive(Default)]
pub struct NullSender;

impl<TB: RuntimeToolbox> ActorRefSender<TB> for NullSender {
  fn send(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    Err(SendError::closed(message))
  }
}
