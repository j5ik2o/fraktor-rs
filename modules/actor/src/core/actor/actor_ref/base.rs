//! Actor reference handle implementation.

#[cfg(test)]
mod tests;

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRefSender, ActorRefSenderSharedGeneric, NullSender, ask_reply_sender::AskReplySenderGeneric},
  },
  error::SendError,
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessageGeneric, AskResponseGeneric, AskResult},
  system::{SystemStateSharedGeneric, SystemStateWeakGeneric},
};

/// Handle used to communicate with an actor instance.
///
/// Uses a weak reference to the system state to avoid circular references
/// when actor references are stored in event stream subscribers.
pub struct ActorRefGeneric<TB: RuntimeToolbox + 'static> {
  pid:    Pid,
  sender: ActorRefSenderSharedGeneric<TB>,
  system: Option<SystemStateWeakGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ActorRefGeneric<TB> {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: T) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, None)
  }

  /// Creates an actor reference backed by the given sender and system state (path-aware).
  #[must_use]
  pub fn with_system<T>(pid: Pid, sender: T, system: &SystemStateSharedGeneric<TB>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, Some(system.downgrade()))
  }

  fn from_parts<T>(pid: Pid, sender: T, system: Option<SystemStateWeakGeneric<TB>>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self { pid, sender: ActorRefSenderSharedGeneric::new(sender), system }
  }

  /// Creates an actor reference from an existing shared sender.
  #[must_use]
  pub fn from_shared(pid: Pid, sender: ActorRefSenderSharedGeneric<TB>, system: &SystemStateSharedGeneric<TB>) -> Self {
    Self { pid, sender, system: Some(system.downgrade()) }
  }

  /// Returns the unique process identifier.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical path of the actor if the system is still available.
  #[must_use]
  pub fn path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|weak| weak.upgrade()).and_then(|system| system.actor_path(&self.pid))
  }

  /// Returns the canonical actor path including authority and UID when available.
  #[must_use]
  pub fn canonical_path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|weak| weak.upgrade()).and_then(|system| system.canonical_actor_path(&self.pid))
  }

  /// Returns the underlying system state if available.
  #[must_use]
  pub(crate) fn system_state(&self) -> Option<SystemStateSharedGeneric<TB>> {
    self.system.as_ref().and_then(|weak| weak.upgrade())
  }

  /// Sends a message to the referenced actor.
  ///
  /// This method delegates to the internal sender which uses interior mutability.
  /// The `&self` signature is intentional as no external mutable borrow is required.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full, closed, or the actor doesn't exist.
  pub fn tell(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    match self.sender.send(message) {
      | Ok(()) => Ok(()),
      | Err(error) => {
        if let Some(system) = self.system.as_ref().and_then(|weak| weak.upgrade()) {
          system.record_send_error(Some(self.pid), &error);
        }
        Err(error)
      },
    }
  }

  /// Sends a request and obtains a future that resolves with the reply.
  ///
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn ask(&self, message: AnyMessageGeneric<TB>) -> Result<AskResponseGeneric<TB>, SendError<TB>>
  where
    TB: 'static, {
    let future = ActorFutureSharedGeneric::<AskResult<TB>, TB>::new();
    let reply_sender = AskReplySenderGeneric::<TB>::new(future.clone());
    let reply_ref = ActorRefGeneric::<TB>::new(self.pid, reply_sender);
    let envelope = message.with_sender(reply_ref.clone());
    self.tell(envelope)?;
    if let Some(system) = self.system.as_ref().and_then(|weak| weak.upgrade()) {
      system.register_ask_future(future.clone());
    }
    Ok(AskResponseGeneric::new(reply_ref, future))
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    Self { pid: Pid::new(0, 0), sender: ActorRefSenderSharedGeneric::new(NullSender), system: None }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for ActorRefGeneric<TB> {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone(), system: self.system.clone() }
  }
}

// SAFETY: `ActorRefGeneric` holds `ArcShared` handles to trait objects that are required to be both
// `Send` and `Sync`. Cloning or dropping the reference does not violate thread-safety guarantees.
unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorRefGeneric<TB> {}

unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorRefGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> fmt::Debug for ActorRefGeneric<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ActorRef").field("pid", &self.pid).finish()
  }
}

impl<TB: RuntimeToolbox + 'static> PartialEq for ActorRefGeneric<TB> {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl<TB: RuntimeToolbox + 'static> Eq for ActorRefGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> Hash for ActorRefGeneric<TB> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid.hash(state);
  }
}

/// Type alias for `ActorRefGeneric` with the default `NoStdToolbox`.
pub type ActorRef = ActorRefGeneric<NoStdToolbox>;
