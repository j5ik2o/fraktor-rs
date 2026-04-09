//! Shared wrapper for actor reference senders.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  system::lock_provider::{DebugSpinLock, DebugSpinLockGuard},
};

/// Shared wrapper for [`ActorRefSender`] with external mutex synchronization.
pub struct ActorRefSenderShared {
  inner: ActorRefSenderLock,
}

enum ActorRefSenderGuard<'a> {
  Builtin(spin::MutexGuard<'a, Box<dyn ActorRefSender>>),
  Debug(DebugSpinLockGuard<'a, Box<dyn ActorRefSender>>),
}

impl core::ops::Deref for ActorRefSenderGuard<'_> {
  type Target = Box<dyn ActorRefSender>;

  fn deref(&self) -> &Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

impl core::ops::DerefMut for ActorRefSenderGuard<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

enum ActorRefSenderLock {
  Builtin(ArcShared<RuntimeMutex<Box<dyn ActorRefSender>>>),
  Debug(ArcShared<DebugSpinLock<Box<dyn ActorRefSender>>>),
}

impl Clone for ActorRefSenderLock {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug(inner) => Self::Debug(inner.clone()),
    }
  }
}

impl ActorRefSenderLock {
  fn builtin(sender: Box<dyn ActorRefSender>) -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(sender)))
  }

  fn debug(sender: Box<dyn ActorRefSender>) -> Self {
    Self::Debug(ArcShared::new(DebugSpinLock::new(sender, "actor_ref_sender_shared.inner")))
  }

  fn lock(&self) -> ActorRefSenderGuard<'_> {
    match self {
      | Self::Builtin(inner) => ActorRefSenderGuard::Builtin(inner.lock()),
      | Self::Debug(inner) => ActorRefSenderGuard::Debug(inner.lock()),
    }
  }
}

impl Clone for ActorRefSenderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl ActorRefSenderShared {
  /// Creates a new shared sender.
  #[must_use]
  pub fn new<S: ActorRefSender + 'static>(sender: S) -> Self {
    Self::from_boxed(Box::new(sender))
  }

  /// Creates a built-in shared sender from an already boxed sender.
  #[must_use]
  pub fn from_boxed(sender: Box<dyn ActorRefSender>) -> Self {
    Self { inner: ActorRefSenderLock::builtin(sender) }
  }

  pub(crate) fn from_boxed_debug(sender: Box<dyn ActorRefSender>) -> Self {
    Self { inner: ActorRefSenderLock::debug(sender) }
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
