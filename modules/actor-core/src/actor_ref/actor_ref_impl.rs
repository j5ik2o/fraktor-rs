//! Actor reference handle implementation.

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{ask_reply_sender::AskReplySender, null_sender::NullSender};
use crate::{
  actor_future::ActorFuture, actor_ref::ActorRefSender, any_message::AnyOwnedMessage, ask_response::AskResponse,
  pid::Pid, send_error::SendError, system_state::ActorSystemState,
};

/// Handle used to communicate with an actor instance.
pub struct ActorRef {
  pid:    Pid,
  sender: ArcShared<dyn ActorRefSender>,
  system: Option<ArcShared<ActorSystemState>>,
}

impl ActorRef {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: ArcShared<T>) -> Self
  where
    T: ActorRefSender + 'static, {
    Self::from_parts(pid, sender, None)
  }

  #[must_use]
  pub(crate) fn with_system<T>(pid: Pid, sender: ArcShared<T>, system: ArcShared<ActorSystemState>) -> Self
  where
    T: ActorRefSender + 'static, {
    Self::from_parts(pid, sender, Some(system))
  }

  fn from_parts<T>(pid: Pid, sender: ArcShared<T>, system: Option<ArcShared<ActorSystemState>>) -> Self
  where
    T: ActorRefSender + 'static, {
    let dyn_sender: ArcShared<dyn ActorRefSender> = sender;
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
  /// Returns an error if the send operation fails.
  pub fn tell(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.sender.send(message)
  }

  /// Sends a request and obtains a future that resolves with the reply.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be sent.
  pub fn ask(&self, message: AnyOwnedMessage) -> Result<AskResponse, SendError> {
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
    let sender: ArcShared<dyn ActorRefSender> = ArcShared::new(NullSender);
    Self { pid: Pid::new(0, 0), sender, system: None }
  }
}

impl Clone for ActorRef {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone(), system: self.system.clone() }
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
