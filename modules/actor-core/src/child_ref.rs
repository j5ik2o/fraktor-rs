//! Handle used by parents to interact with child actors.

use core::fmt;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_ref::ActorRef, any_message::AnyMessage, ask_response::AskResponse, pid::Pid,
  send_error::SendError, system_message::SystemMessage, system_state::SystemState,
};

/// Provides typed accessors to a child actor owned by a parent.
pub struct ChildRef<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  actor:  ActorRef<TB>,
  system: ArcShared<SystemState<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ChildRef<TB> {
  pub(crate) const fn new(actor: ActorRef<TB>, system: ArcShared<SystemState<TB>>) -> Self {
    Self { actor, system }
  }

  /// Returns the pid of the child actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.actor.pid()
  }

  /// Returns the underlying actor reference.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef<TB> {
    &self.actor
  }

  /// Sends a user message to the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the mailbox cannot accept the message.
  pub fn tell(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    self.actor.tell(message)
  }

  /// Sends a request to the child actor and returns the associated ask response.
  ///
  /// # Errors
  ///
  /// Returns an error when the message cannot be enqueued.
  pub fn ask(&self, message: AnyMessage<TB>) -> Result<AskResponse<TB>, SendError<TB>> {
    self.actor.ask(message)
  }

  /// Sends a stop signal to the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop signal cannot be delivered.
  pub fn stop(&self) -> Result<(), SendError<TB>> {
    self.system.send_system_message(self.pid(), SystemMessage::Stop)
  }

  /// Suspends the child mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the suspend signal cannot be delivered.
  pub fn suspend(&self) -> Result<(), SendError<TB>> {
    self.system.send_system_message(self.pid(), SystemMessage::Suspend)
  }

  /// Resumes the child mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the resume signal cannot be delivered.
  pub fn resume(&self) -> Result<(), SendError<TB>> {
    self.system.send_system_message(self.pid(), SystemMessage::Resume)
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for ChildRef<TB> {
  fn clone(&self) -> Self {
    Self { actor: self.actor.clone(), system: self.system.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> fmt::Debug for ChildRef<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ChildRef").field("pid", &self.pid()).finish()
  }
}

impl<TB: RuntimeToolbox + 'static> PartialEq for ChildRef<TB> {
  fn eq(&self, other: &Self) -> bool {
    self.pid() == other.pid()
  }
}

impl<TB: RuntimeToolbox + 'static> Eq for ChildRef<TB> {}
