//! Shared wrapper for actor reference senders.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedAccess;

use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  system::lock_provider::SharedLock,
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
  /// Creates a new shared sender backed by the built-in lock.
  #[must_use]
  pub fn new_with_builtin_lock<S: ActorRefSender + 'static>(sender: S) -> Self {
    Self::from_shared_lock(SharedLock::builtin(Box::new(sender)))
  }

  /// Creates a shared sender from an already constructed lock.
  #[must_use]
  pub(crate) fn from_shared_lock(inner: SharedLock<Box<dyn ActorRefSender>>) -> Self {
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
    let outcome = {
      let mut sender = self.inner.lock();
      sender.send(message)
    };

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
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorRefSender>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
