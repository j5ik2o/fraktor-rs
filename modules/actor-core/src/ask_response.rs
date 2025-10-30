//! Response handle returned by [`ActorRef::ask`](crate::actor_ref::ActorRef::ask).

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{actor_future::ActorFuture, actor_ref::ActorRef, any_message::AnyOwnedMessage};

/// Holds the reply handle and future associated with an ask request.
pub struct AskResponse {
  reply_to: ActorRef,
  future:   ArcShared<ActorFuture<AnyOwnedMessage>>,
}

impl AskResponse {
  /// Creates a new ask response handle.
  #[must_use]
  pub const fn new(reply_to: ActorRef, future: ArcShared<ActorFuture<AnyOwnedMessage>>) -> Self {
    Self { reply_to, future }
  }

  /// Returns the reply handle exposed to the caller.
  #[must_use]
  pub const fn reply_to(&self) -> &ActorRef {
    &self.reply_to
  }

  /// Returns a reference to the future that resolves with the response message.
  #[must_use]
  pub const fn future(&self) -> &ArcShared<ActorFuture<AnyOwnedMessage>> {
    &self.future
  }

  /// Decomposes the response into its parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorRef, ArcShared<ActorFuture<AnyOwnedMessage>>) {
    (self.reply_to, self.future)
  }
}
