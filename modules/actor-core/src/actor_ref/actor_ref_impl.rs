//! Actor reference handle implementation.

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  actor_future::ActorFuture,
  actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender, null_sender::NullSender},
  any_message::AnyMessage,
  ask_response::AskResponse,
  pid::Pid,
  send_error::SendError,
};

/// Handle used to communicate with an actor instance.
pub struct ActorRef<TB: RuntimeToolbox = NoStdToolbox> {
  pid:    Pid,
  sender: ArcShared<dyn ActorRefSender<TB>>,
}

impl<TB: RuntimeToolbox> ActorRef<TB> {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    Self::from_parts(pid, sender)
  }

  fn from_parts<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender<TB> + 'static, {
    let dyn_sender: ArcShared<dyn ActorRefSender<TB>> = sender;
    Self { pid, sender: dyn_sender }
  }

  /// Returns the unique process identifier.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Sends a message to the referenced actor.
  pub fn tell(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    self.sender.send(message)
  }

  /// Sends a request and obtains a future that resolves with the reply.
  pub fn ask(&self, message: AnyMessage<TB>) -> Result<AskResponse<TB>, SendError<TB>>
  where
    TB: 'static, {
    let future = ArcShared::new(ActorFuture::new());
    let reply_sender = ArcShared::new(AskReplySender::new(future.clone()));
    let reply_ref = ActorRef::new(self.pid, reply_sender);
    let envelope = message.with_reply_to(reply_ref.clone());
    self.tell(envelope)?;
    Ok(AskResponse::new(reply_ref, future))
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    let sender = ArcShared::new(NullSender::default());
    let dyn_sender: ArcShared<dyn ActorRefSender<TB>> = sender;
    Self { pid: Pid::new(0, 0), sender: dyn_sender }
  }
}

impl<TB: RuntimeToolbox> Clone for ActorRef<TB> {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone() }
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

#[cfg(test)]
mod tests;
