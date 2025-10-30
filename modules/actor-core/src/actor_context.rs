//! Actor execution context utilities.

use crate::{
  actor_ref::ActorRef, any_message::AnyMessage, pid::Pid, props::Props, send_error::SendError, spawn_error::SpawnError,
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
  pub const fn new(system: &'a ActorSystem, pid: Pid) -> Self {
    Self { system, pid, reply_to: None }
  }

  /// Returns a reference to the actor system.
  #[must_use]
  pub const fn system(&self) -> &'a ActorSystem {
    self.system
  }

  /// Returns the pid of the running actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the reply target if supplied by the message envelope.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef> {
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

  /// Returns an [`ActorRef`] pointing to the running actor.
  #[must_use]
  pub fn self_ref(&self) -> ActorRef {
    self.system.actor_ref(self.pid).expect("actor reference must exist for running context")
  }

  /// Sends a reply to the caller if a reply target is present.
  ///
  /// # Errors
  ///
  /// Returns an error if no reply target is set or if the send operation fails.
  pub fn reply(&self, message: AnyMessage) -> Result<(), SendError> {
    match self.reply_to.as_ref() {
      | Some(target) => target.tell(message),
      | None => Err(SendError::no_recipient(message)),
    }
  }

  /// Requests the actor system to spawn a child actor using the provided props.
  ///
  /// # Errors
  ///
  /// Returns an error if actor spawning fails.
  pub fn spawn_child(&self, props: &Props) -> Result<ActorRef, SpawnError> {
    self.system.spawn_child(self.pid, props)
  }
}
