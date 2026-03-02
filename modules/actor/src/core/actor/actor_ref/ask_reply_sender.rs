//! Sender used to deliver ask responses back to the awaiting future.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::{
  actor::actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskResult},
};

/// Sender that completes the associated `ActorFuture` when a reply arrives.
pub struct AskReplySender {
  future: ActorFutureShared<AskResult>,
}

impl AskReplySender {
  /// Creates a new reply sender.
  #[must_use]
  pub const fn new(future: ActorFutureShared<AskResult>) -> Self {
    Self { future }
  }
}

impl ActorRefSender for AskReplySender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    // Lock, complete with Ok, then wake outside the lock to avoid deadlock.
    let waker = self.future.with_write(|af| af.complete(Ok(message)));
    if let Some(w) = waker {
      w.wake();
    }
    Ok(SendOutcome::Delivered)
  }
}
