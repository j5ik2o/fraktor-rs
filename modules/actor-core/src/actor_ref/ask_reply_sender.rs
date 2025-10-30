//! Internal sender for ask-reply pattern.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_future::ActorFuture, actor_ref::ActorRefSender, any_message::AnyOwnedMessage, send_error::SendError,
};

pub(super) struct AskReplySender {
  future: ArcShared<ActorFuture<AnyOwnedMessage>>,
}

impl AskReplySender {
  pub(super) const fn new(future: ArcShared<ActorFuture<AnyOwnedMessage>>) -> Self {
    Self { future }
  }
}

impl ActorRefSender for AskReplySender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.future.complete(message);
    Ok(())
  }
}
