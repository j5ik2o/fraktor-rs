//! Actor reference handle implementation.

#[cfg(test)]
mod tests;

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRefSender, NullSender, ask_reply_sender::AskReplySenderGeneric},
  },
  error::SendError,
  futures::ActorFuture,
  messaging::{AnyMessageGeneric, AskResponseGeneric},
  system::SystemStateGeneric,
};

/// Handle used to communicate with an actor instance.
pub struct ActorRefGeneric<TB: RuntimeToolbox> {
  pid:    Pid,
  sender: ArcShared<dyn ActorRefSender<TB>>,
  system: Option<ArcShared<SystemStateGeneric<TB>>>,
}

impl<TB: RuntimeToolbox> ActorRefGeneric<TB> {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, None)
  }

  /// Creates an actor reference backed by the given sender and system state (path-aware).
  #[must_use]
  pub fn with_system<T>(pid: Pid, sender: ArcShared<T>, system: ArcShared<SystemStateGeneric<TB>>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, Some(system))
  }

  fn from_parts<T>(pid: Pid, sender: ArcShared<T>, system: Option<ArcShared<SystemStateGeneric<TB>>>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    let dyn_sender: ArcShared<dyn ActorRefSender<TB>> = sender;
    Self { pid, sender: dyn_sender, system }
  }

  /// Returns the unique process identifier.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical path of the actor if the system is still available.
  #[must_use]
  pub fn path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|system| system.actor_path(&self.pid))
  }

  /// Returns the canonical actor path including authority and UID when available.
  #[must_use]
  pub fn canonical_path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|system| system.canonical_actor_path(&self.pid))
  }

  /// Sends a message to the referenced actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full, closed, or the actor doesn't exist.
  pub fn tell(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    match self.sender.send(message) {
      | Ok(()) => Ok(()),
      | Err(error) => {
        if let Some(system) = &self.system {
          system.record_send_error(Some(self.pid), &error);
        }
        Err(error)
      },
    }
  }

  /// Sends a request and obtains a future that resolves with the reply.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn ask(&self, message: AnyMessageGeneric<TB>) -> Result<AskResponseGeneric<TB>, SendError<TB>>
  where
    TB: 'static, {
    let future = ArcShared::new(ActorFuture::new());
    let reply_sender = ArcShared::new(AskReplySenderGeneric::<TB>::new(future.clone()));
    let reply_ref = ActorRefGeneric::<TB>::new(self.pid, reply_sender);
    let envelope = message.with_reply_to(reply_ref.clone());
    self.tell(envelope)?;
    if let Some(system) = &self.system {
      system.register_ask_future(future.clone());
    }
    Ok(AskResponseGeneric::new(reply_ref, future))
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    let sender = ArcShared::new(NullSender);
    let dyn_sender: ArcShared<dyn ActorRefSender<TB>> = sender;
    Self { pid: Pid::new(0, 0), sender: dyn_sender, system: None }
  }
}

impl<TB: RuntimeToolbox> Clone for ActorRefGeneric<TB> {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone(), system: self.system.clone() }
  }
}

// SAFETY: `ActorRefGeneric` holds `ArcShared` handles to trait objects that are required to be both
// `Send` and `Sync`. Cloning or dropping the reference does not violate thread-safety guarantees.
unsafe impl<TB: RuntimeToolbox> Send for ActorRefGeneric<TB> {}

unsafe impl<TB: RuntimeToolbox> Sync for ActorRefGeneric<TB> {}

impl<TB: RuntimeToolbox> fmt::Debug for ActorRefGeneric<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ActorRef").field("pid", &self.pid).finish()
  }
}

impl<TB: RuntimeToolbox> PartialEq for ActorRefGeneric<TB> {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl<TB: RuntimeToolbox> Eq for ActorRefGeneric<TB> {}

impl<TB: RuntimeToolbox> Hash for ActorRefGeneric<TB> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid.hash(state);
  }
}

/// Type alias for `ActorRefGeneric` with the default `NoStdToolbox`.
pub type ActorRef = ActorRefGeneric<NoStdToolbox>;
