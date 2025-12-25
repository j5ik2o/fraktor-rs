//! Sender used to deliver ask responses back to the awaiting future.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::SharedAccess,
};

use crate::core::{
  actor::actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessageGeneric, AskResult},
};

/// Sender that completes the associated `ActorFuture` when a reply arrives.
pub struct AskReplySenderGeneric<TB: RuntimeToolbox + 'static> {
  future: ActorFutureSharedGeneric<AskResult<TB>, TB>,
}

/// Type alias for the default `NoStdToolbox`-backed reply sender.
pub type AskReplySender = AskReplySenderGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskReplySenderGeneric<TB> {
  /// Creates a new reply sender.
  #[must_use]
  pub const fn new(future: ActorFutureSharedGeneric<AskResult<TB>, TB>) -> Self {
    Self { future }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for AskReplySenderGeneric<TB> {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    // Lock, complete with Ok, then wake outside the lock to avoid deadlock.
    let waker = self.future.with_write(|af| af.complete(Ok(message)));
    if let Some(w) = waker {
      w.wake();
    }
    Ok(SendOutcome::Delivered)
  }
}
