//! ActorRef handle used for message dispatching.

use crate::{
  actor_future::ActorFuture,
  any_owned_message::AnyOwnedMessage,
  pid::Pid,
  send_error::SendError,
  system::{SystemShared, enqueue_system, enqueue_user},
};

/// Reference to a running actor managed by [`ActorSystem`](crate::system::ActorSystem).
#[derive(Clone)]
pub struct ActorRef {
  pid:    Pid,
  system: SystemShared,
}

impl ActorRef {
  /// Creates a new handle backed by the shared system state.
  pub(crate) fn new(pid: Pid, system: SystemShared) -> Self {
    Self { pid, system }
  }

  /// Returns the referenced PID.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Sends a user message to the actor.
  pub fn tell(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    enqueue_user(&self.system, self.pid, message)
  }

  /// Sends a system message to the actor.
  pub fn tell_system(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    enqueue_system(&self.system, self.pid, message)
  }

  /// Sends a request expecting a reply. Current implementation forwards the message using
  /// `tell` semantics and returns a pending future for callers that wish to coordinate manually.
  pub fn ask(&self, message: AnyOwnedMessage) -> Result<ActorFuture<AnyOwnedMessage>, SendError> {
    enqueue_user(&self.system, self.pid, message)?;
    Ok(ActorFuture::pending())
  }
}

impl PartialEq for ActorRef {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl Eq for ActorRef {}
