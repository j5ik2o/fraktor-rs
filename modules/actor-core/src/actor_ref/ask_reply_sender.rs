//! Internal sender for ask-reply pattern.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{actor_future::ActorFuture, actor_ref::ActorRefSender, any_message::AnyMessage, send_error::SendError};

pub(super) struct AskReplySender {
  future: ArcShared<ActorFuture<AnyMessage>>,
}

impl AskReplySender {
  pub(super) const fn new(future: ArcShared<ActorFuture<AnyMessage>>) -> Self {
    Self { future }
  }
}

impl ActorRefSender for AskReplySender {
  fn send(&self, message: AnyMessage) -> Result<(), SendError> {
    self.future.complete(message);
    Ok(())
  }
}
