//! Null sender that rejects all messages.

use crate::{actor_ref::ActorRefSender, any_message::AnyOwnedMessage, send_error::SendError};

pub(super) struct NullSender;

impl ActorRefSender for NullSender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    Err(SendError::no_recipient(message))
  }
}
