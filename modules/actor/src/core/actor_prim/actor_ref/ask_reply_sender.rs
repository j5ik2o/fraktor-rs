//! Sender used to deliver ask responses back to the awaiting future.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::sync_mutex_like::SyncMutexLike,
};

use crate::core::{
  actor_prim::actor_ref::ActorRefSender, error::SendError, futures::ActorFutureShared, messaging::AnyMessageGeneric,
};

/// Sender that completes the associated `ActorFuture` when a reply arrives.
pub struct AskReplySenderGeneric<TB: RuntimeToolbox + 'static> {
  future: ActorFutureShared<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for the default `NoStdToolbox`-backed reply sender.
pub type AskReplySender = AskReplySenderGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskReplySenderGeneric<TB> {
  /// Creates a new reply sender.
  #[must_use]
  pub const fn new(future: ActorFutureShared<AnyMessageGeneric<TB>, TB>) -> Self {
    Self { future }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for AskReplySenderGeneric<TB> {
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    // Lock, complete, then wake outside the lock to avoid deadlock.
    let waker = self.future.lock().complete(message);
    if let Some(w) = waker {
      w.wake();
    }
    Ok(())
  }
}
