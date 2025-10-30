//! Actor execution context utilities.

use crate::{
  actor_ref::ActorRef,
  any_message::AnyOwnedMessage,
  pid::Pid,
  props::Props,
  send_error::SendError,
  spawn_error::SpawnError,
  system::ActorSystem,
};

/// Provides contextual APIs while handling a message.
pub struct ActorContext<'a> {
  system:   &'a ActorSystem,
  pid:      Pid,
  reply_to: Option<ActorRef>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context placeholder.
  #[must_use]
  pub fn new(system: &'a ActorSystem, pid: Pid) -> Self {
    Self { system, pid, reply_to: None }
  }

  /// Returns a reference to the actor system.
  #[must_use]
  pub fn system(&self) -> &'a ActorSystem {
    self.system
  }

  /// Returns the pid of the running actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the reply target if supplied by the message envelope.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to.as_ref()
  }

  /// Sets the reply target (used internally by the system).
  pub fn set_reply_to(&mut self, reply_to: Option<ActorRef>) {
    self.reply_to = reply_to;
  }

  /// Clears the reply target (used after message processing completes).
  pub fn clear_reply_to(&mut self) {
    self.reply_to = None;
  }

  /// Sends a reply to the caller if a reply target is present.
  pub fn reply(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    match self.reply_to.as_ref() {
      Some(target) => target.tell(message),
      None => Err(SendError::no_recipient(message)),
    }
  }

  /// Requests the actor system to spawn a child actor using the provided props.
  pub fn spawn_child(&self, props: Props) -> Result<ActorRef, SpawnError> {
    self.system.spawn_child(self.pid, props)
  }
}
