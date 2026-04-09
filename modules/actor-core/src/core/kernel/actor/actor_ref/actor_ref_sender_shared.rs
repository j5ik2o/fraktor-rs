//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

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
  Debug { inner: ArcShared<DebugSpinLock<Box<dyn ActorRefSender>>>, active: ArcShared<AtomicBool> },
}

impl Clone for ActorRefSenderLock {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug { inner, active } => Self::Debug { inner: inner.clone(), active: active.clone() },
    }
  }
}

impl ActorRefSenderLock {
  fn builtin(sender: Box<dyn ActorRefSender>) -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(sender)))
  }

  fn debug(sender: Box<dyn ActorRefSender>) -> Self {
    Self::Debug {
      inner:  ArcShared::new(DebugSpinLock::new(sender, "actor_ref_sender_shared.inner")),
      active: ArcShared::new(AtomicBool::new(false)),
    }
  }

  fn lock(&self) -> ActorRefSenderGuard<'_> {
    match self {
      | Self::Builtin(inner) => ActorRefSenderGuard::Builtin(inner.lock()),
      | Self::Debug { inner, .. } => ActorRefSenderGuard::Debug(inner.lock()),
    }
  }

  fn enter_debug_scope(&self) -> Option<DebugSendScope> {
    match self {
      | Self::Builtin(_) => None,
      | Self::Debug { active, .. } => {
        assert!(
          !active.swap(true, Ordering::AcqRel),
          "debug actor lock provider detected nested tell on the same thread"
        );
        Some(DebugSendScope { active: active.clone() })
      },
    }
  }
}

struct DebugSendScope {
  active: ArcShared<AtomicBool>,
}

impl Drop for DebugSendScope {
  fn drop(&mut self) {
    self.active.store(false, Ordering::Release);
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
    let _debug_scope = self.inner.enter_debug_scope();
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
