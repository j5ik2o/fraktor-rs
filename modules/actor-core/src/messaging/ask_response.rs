//! Holds reply handle and future associated with an ask request.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_prim::actor_ref::ActorRef, futures::ActorFuture, messaging::AnyMessage,
};

/// Combines the reply handle and future returned by `ActorRef::ask`.
pub struct AskResponse<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  reply_to: ActorRef<TB>,
  future:   ArcShared<ActorFuture<AnyMessage<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> AskResponse<TB> {
  /// Creates a new ask response handle.
  #[must_use]
  pub const fn new(reply_to: ActorRef<TB>, future: ArcShared<ActorFuture<AnyMessage<TB>, TB>>) -> Self {
    Self { reply_to, future }
  }

  /// Returns the reply handle exposed to the caller.
  #[must_use]
  pub const fn reply_to(&self) -> &ActorRef<TB> {
    &self.reply_to
  }

  /// Returns a reference to the future that resolves with the response message.
  #[must_use]
  pub const fn future(&self) -> &ArcShared<ActorFuture<AnyMessage<TB>, TB>> {
    &self.future
  }

  /// Decomposes the response into its parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorRef<TB>, ArcShared<ActorFuture<AnyMessage<TB>, TB>>) {
    (self.reply_to, self.future)
  }
}
