//! Null sender that rejects all messages.

use crate::{actor_ref::ActorRefSender, any_message::AnyMessage, send_error::SendError};

pub(super) struct NullSender;

impl ActorRefSender for NullSender {
  fn send(&self, message: AnyMessage) -> Result<(), SendError> {
    Err(SendError::no_recipient(message))
  }
}
