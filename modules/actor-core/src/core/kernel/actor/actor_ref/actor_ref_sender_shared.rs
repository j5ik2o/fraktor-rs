//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use crate::core::kernel::actor::{
  actor_ref::{ActorRefSender, SendOutcome},
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
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(sender: Box<dyn ActorRefSender>) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(sender))
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
