//! Holds sender handle and future associated with an ask request.

#[cfg(test)]
mod tests;

use crate::core::kernel::{
  actor::actor_ref::ActorRef,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskError},
};

/// Result type returned by ask operations.
///
/// Contains either the successful reply message or an [`AskError`] indicating
/// the failure reason (timeout, dead letter, or send failure).
pub type AskResult = Result<AnyMessage, AskError>;

/// Combines the sender handle and future returned by `ActorRef::ask`.
pub struct AskResponse {
  sender: ActorRef,
  future: ActorFutureShared<AskResult>,
}

impl AskResponse {
  /// Creates a new ask response handle.
  #[must_use]
  pub const fn new(sender: ActorRef, future: ActorFutureShared<AskResult>) -> Self {
    Self { sender, future }
  }

  /// Returns the sender handle exposed to the caller.
  #[must_use]
  pub const fn sender(&self) -> &ActorRef {
    &self.sender
  }

  /// Returns a reference to the shared future that resolves with the response result.
  #[must_use]
  pub const fn future(&self) -> &ActorFutureShared<AskResult> {
    &self.future
  }

  /// Decomposes the response into its parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorRef, ActorFutureShared<AskResult>) {
    (self.sender, self.future)
  }
}
