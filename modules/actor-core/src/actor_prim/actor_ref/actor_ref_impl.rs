//! Actor reference handle implementation.

#[cfg(test)]
mod tests;

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use cellactor_utils_core_rs::sync::{ArcShared, NoStdToolbox};

use crate::{
  RuntimeToolbox,
  actor_prim::{
    actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender, null_sender::NullSender},
    pid::Pid,
  },
  error::SendError,
  futures::ActorFuture,
  messaging::{AnyMessage, AskResponse},
  system::SystemState,
};

/// Handle used to communicate with an actor instance.
pub struct ActorRef<TB: RuntimeToolbox = NoStdToolbox> {
  pid:    Pid,
  sender: ArcShared<dyn ActorRefSender<TB>>,
  system: Option<ArcShared<SystemState<TB>>>,
}

impl<TB: RuntimeToolbox> ActorRef<TB> {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, None)
  }

  pub(crate) fn with_system<T>(pid: Pid, sender: ArcShared<T>, system: ArcShared<SystemState<TB>>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender, Some(system))
  }

  fn from_parts<T>(pid: Pid, sender: ArcShared<T>, system: Option<ArcShared<SystemState<TB>>>) -> Self
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

  /// Sends a message to the referenced actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full, closed, or the actor doesn't exist.
  pub fn tell(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
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
  pub fn ask(&self, message: AnyMessage<TB>) -> Result<AskResponse<TB>, SendError<TB>>
  where
    TB: 'static, {
    let future = ArcShared::new(ActorFuture::new());
    let reply_sender = ArcShared::new(AskReplySender::new(future.clone()));
    let reply_ref = ActorRef::new(self.pid, reply_sender);
    let envelope = message.with_reply_to(reply_ref.clone());
    self.tell(envelope)?;
    if let Some(system) = &self.system {
      system.register_ask_future(future.clone());
    }
    Ok(AskResponse::new(reply_ref, future))
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    let sender = ArcShared::new(NullSender::default());
    let dyn_sender: ArcShared<dyn ActorRefSender<TB>> = sender;
    Self { pid: Pid::new(0, 0), sender: dyn_sender, system: None }
  }
}

impl<TB: RuntimeToolbox> Clone for ActorRef<TB> {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone(), system: self.system.clone() }
  }
}

// SAFETY: `ActorRef` holds `ArcShared` handles to trait objects that are required to be both `Send`
// and `Sync`. Cloning or dropping the reference does not violate thread-safety guarantees.
unsafe impl<TB: RuntimeToolbox> Send for ActorRef<TB> {}

unsafe impl<TB: RuntimeToolbox> Sync for ActorRef<TB> {}

impl<TB: RuntimeToolbox> fmt::Debug for ActorRef<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ActorRef").field("pid", &self.pid).finish()
  }
}

impl<TB: RuntimeToolbox> PartialEq for ActorRef<TB> {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl<TB: RuntimeToolbox> Eq for ActorRef<TB> {}

impl<TB: RuntimeToolbox> Hash for ActorRef<TB> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid.hash(state);
  }
}
