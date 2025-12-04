//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
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
    // ロック解放後にアウトカムを適用し、再入によるデッドロックを防ぐ
    let outcome = self.inner.with_write(|sender| sender.send(message));

    // Apply outcome after releasing lock to allow re-entrant sends
    match outcome? {
      | SendOutcome::Delivered => {},
      | SendOutcome::Schedule(task) => task(),
    }
    Ok(())
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn ActorRefSender<TB>>> for ActorRefSenderSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorRefSender<TB>>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorRefSender<TB>>) -> R) -> R {
    self.inner.with_write(f)
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type ActorRefSenderShared = ActorRefSenderSharedGeneric<NoStdToolbox>;
