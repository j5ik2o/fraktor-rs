//! Shared wrapper for actor reference senders.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider, SenderLockCell},
};

/// Shared wrapper for [`ActorRefSender`] with external mutex synchronization.
pub struct ActorRefSenderShared {
  inner:                 SenderLockCell,
  runtime_lock_provider: ArcShared<dyn ActorRuntimeLockProvider>,
}

impl Clone for ActorRefSenderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), runtime_lock_provider: self.runtime_lock_provider.clone() }
  }
}

impl ActorRefSenderShared {
  /// Creates a new shared sender.
  #[must_use]
  pub fn new<S: ActorRefSender + 'static>(sender: S) -> Self {
    Self::new_with_provider(sender, BuiltinSpinRuntimeLockProvider::shared())
  }

  /// Creates a new shared sender using the given provider.
  #[must_use]
  pub fn new_with_provider<S: ActorRefSender + 'static>(
    sender: S,
    provider: ArcShared<dyn ActorRuntimeLockProvider>,
  ) -> Self {
    let boxed: Box<dyn ActorRefSender> = Box::new(sender);
    Self::from_cell(provider.new_sender_cell(boxed), provider)
  }

  /// Builds a shared wrapper from an already materialized provider cell.
  #[must_use]
  pub const fn from_cell(
    inner: SenderLockCell,
    runtime_lock_provider: ArcShared<dyn ActorRuntimeLockProvider>,
  ) -> Self {
    Self { inner, runtime_lock_provider }
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
    let outcome = self.inner.with_write(|sender| sender.send(message));

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
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorRefSender>) -> R) -> R {
    self.inner.with_write(f)
  }
}
