//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  actor_prim::actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessageGeneric,
};

/// Shared wrapper for [`ActorRefSender`] with external mutex synchronization.
pub struct ActorRefSenderSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn ActorRefSender<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for ActorRefSenderSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSenderSharedGeneric<TB> {
  /// Creates a new shared sender.
  #[must_use]
  pub fn new<S: ActorRefSender<TB> + 'static>(sender: S) -> Self {
    let boxed: Box<dyn ActorRefSender<TB>> = Box::new(sender);
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(boxed)) }
  }

  /// Sends a message through the wrapped sender.
  ///
  /// This method acquires an internal lock and delegates to the wrapped sender.
  /// The `&self` signature is intentional as the mutex provides interior mutability.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    let outcome = {
      let mut guard = self.inner.lock();
      guard.send(message)
    }?;

    match outcome {
      | SendOutcome::Delivered => Ok(()),
      | SendOutcome::Schedule(task) => {
        task();
        Ok(())
      },
    }
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type ActorRefSenderShared = ActorRefSenderSharedGeneric<NoStdToolbox>;
