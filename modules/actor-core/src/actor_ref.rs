//! Actor reference handle.

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_future::ActorFuture,
  any_message::AnyOwnedMessage,
  ask_response::AskResponse,
  pid::Pid,
  send_error::SendError,
};

/// Trait implemented by mailbox endpoints that accept [`AnyOwnedMessage`] instances.
pub trait ActorRefSender: Send + Sync {
  /// Enqueues the message into the underlying mailbox.
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError>;
}

/// Handle used to communicate with an actor instance.
pub struct ActorRef {
  pid:    Pid,
  sender: ArcShared<dyn ActorRefSender>,
}

impl ActorRef {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender + 'static, {
    let dyn_sender: ArcShared<dyn ActorRefSender> = sender;
    Self { pid, sender: dyn_sender }
  }

  /// Returns the unique process identifier.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Sends a message to the referenced actor.
  pub fn tell(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.sender.send(message)
  }

  /// Sends a request and obtains a future that resolves with the reply.
  pub fn ask(&self, message: AnyOwnedMessage) -> Result<AskResponse, SendError> {
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
    let sender: ArcShared<dyn ActorRefSender> = ArcShared::new(NullSender);
    Self { pid: Pid::new(0, 0), sender }
  }
}

impl Clone for ActorRef {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone() }
  }
}

impl fmt::Debug for ActorRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ActorRef").field("pid", &self.pid).finish()
  }
}

impl PartialEq for ActorRef {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl Eq for ActorRef {}

impl Hash for ActorRef {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid.hash(state);
  }
}

struct AskReplySender {
  future: ArcShared<ActorFuture<AnyOwnedMessage>>,
}

impl AskReplySender {
  fn new(future: ArcShared<ActorFuture<AnyOwnedMessage>>) -> Self {
    Self { future }
  }
}

impl ActorRefSender for AskReplySender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.future.complete(message);
    Ok(())
  }
}

struct NullSender;

impl ActorRefSender for NullSender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    Err(SendError::no_recipient(message))
  }
}

#[cfg(test)]
mod tests;
