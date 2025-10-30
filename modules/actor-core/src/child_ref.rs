use core::fmt;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_ref::ActorRef, any_message::AnyMessage, ask_response::AskResponse, pid::Pid, send_error::SendError,
  system_message::SystemMessage, system_state::ActorSystemState,
};

/// Handle used by parents to manage child actors.
pub struct ChildRef {
  actor:  ActorRef,
  system: ArcShared<ActorSystemState>,
}

impl ChildRef {
  pub(crate) fn new(actor: ActorRef, system: ArcShared<ActorSystemState>) -> Self {
    Self { actor, system }
  }

  /// Returns the child pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.actor.pid()
  }

  /// Returns the underlying actor reference.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor
  }

  /// Sends a user message to the child.
  ///
  /// # Errors
  ///
  /// Returns an error if enqueueing the message fails.
  pub fn tell(&self, message: AnyMessage) -> Result<(), SendError> {
    self.actor.tell(message)
  }

  /// Sends a request to the child and returns an ask response handle.
  pub fn ask(&self, message: AnyMessage) -> Result<AskResponse, SendError> {
    self.actor.ask(message)
  }

  /// Sends a stop signal to the child.
  pub fn stop(&self) -> Result<(), SendError> {
    self.system.send_system_message(self.pid(), SystemMessage::Stop)
  }

  /// Suspends the child mailbox.
  pub fn suspend(&self) -> Result<(), SendError> {
    self.system.send_system_message(self.pid(), SystemMessage::Suspend)
  }

  /// Resumes the child mailbox.
  pub fn resume(&self) -> Result<(), SendError> {
    self.system.send_system_message(self.pid(), SystemMessage::Resume)
  }
}

impl Clone for ChildRef {
  fn clone(&self) -> Self {
    Self { actor: self.actor.clone(), system: self.system.clone() }
  }
}

impl fmt::Debug for ChildRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ChildRef").field("pid", &self.pid()).finish()
  }
}

impl PartialEq for ChildRef {
  fn eq(&self, other: &Self) -> bool {
    self.pid() == other.pid()
  }
}

impl Eq for ChildRef {}
