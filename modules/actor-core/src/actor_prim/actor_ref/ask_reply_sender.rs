//! Sender used to deliver ask responses back to the awaiting future.

#[cfg(test)]
mod tests;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  RuntimeToolbox, actor_prim::actor_ref::ActorRefSender, error::SendError, futures::ActorFuture, messaging::AnyMessage,
};

/// Sender that completes the associated `ActorFuture` when a reply arrives.
pub struct AskReplySender<TB: RuntimeToolbox + 'static> {
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
