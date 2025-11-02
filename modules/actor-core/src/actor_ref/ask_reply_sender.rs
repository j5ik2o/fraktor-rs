//! Sender used to deliver ask responses back to the awaiting future.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_future::ActorFuture, actor_ref::actor_ref_sender::ActorRefSender,
  any_message::AnyMessage, send_error::SendError,
};

/// Sender that completes the associated `ActorFuture` when a reply arrives.
pub struct AskReplySender<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  future: ArcShared<ActorFuture<AnyMessage<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> AskReplySender<TB> {
  /// Creates a new reply sender.
  #[must_use]
  pub const fn new(future: ArcShared<ActorFuture<AnyMessage<TB>, TB>>) -> Self {
    Self { future }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for AskReplySender<TB> {
  fn send(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    self.future.complete(message);
    Ok(())
  }
}

#[cfg(test)]
mod tests;
