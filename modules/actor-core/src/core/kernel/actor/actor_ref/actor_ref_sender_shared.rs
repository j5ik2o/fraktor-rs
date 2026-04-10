//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use crate::core::kernel::actor::{
  actor_ref::{ActorRefSender, NullSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

/// Shared wrapper for [`ActorRefSender`] with external mutex synchronization.
pub struct ActorRefSenderShared {
  inner: SharedLock<Box<dyn ActorRefSender>>,
}

impl Clone for ActorRefSenderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl ActorRefSenderShared {
  #[must_use]
  pub(crate) fn from_builtin_sender(sender: Box<dyn ActorRefSender>) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<Box<dyn ActorRefSender>>>(sender))
  }

  #[must_use]
  pub(crate) fn null_sender() -> Self {
    Self::from_builtin_sender(Box::new(NullSender))
  }

  /// Creates a new shared sender backed by the built-in lock.
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn new_with_builtin_lock<S: ActorRefSender + 'static>(sender: S) -> Self {
    Self::from_builtin_sender(Box::new(sender))
  }

  /// Creates a shared sender from an already constructed shared lock.
  #[must_use]
  pub fn from_shared_lock(inner: SharedLock<Box<dyn ActorRefSender>>) -> Self {
    Self { inner }
  }

  /// Sends a message through the wrapped sender.
  ///
  /// This method acquires an internal lock and delegates to the wrapped sender.
  /// The `&self` signature is intentional as the mutex provides interior mutability.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn send(&mut self, message: AnyMessage) -> Result<(), SendError> {
    // ロック解放後にアウトカムを適用し、再入によるデッドロックを防ぐ
    let outcome = self.inner.with_lock(|sender| sender.send(message));

    // Apply outcome after releasing lock to allow re-entrant sends
    match outcome? {
      | SendOutcome::Delivered => {},
      | SendOutcome::Schedule(task) => task(),
    }
    Ok(())
  }
}

impl SharedAccess<Box<dyn ActorRefSender>> for ActorRefSenderShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorRefSender>) -> R) -> R {
    self.inner.with_read(|guard| f(guard))
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorRefSender>) -> R) -> R {
    self.inner.with_lock(|guard| f(guard))
  }
}
