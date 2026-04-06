//! Handle used by parents to interact with child actors.

use core::{fmt, time::Duration};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    error::SendError,
    messaging::{AnyMessage, AskResponse, system_message::SystemMessage},
  },
  system::state::SystemStateShared,
};

/// Provides typed accessors to a child actor owned by a parent.
pub struct ChildRef {
  actor:  ActorRef,
  system: SystemStateShared,
}

impl ChildRef {
  pub(crate) const fn new(actor: ActorRef, system: SystemStateShared) -> Self {
    Self { actor, system }
  }

  /// Returns the pid of the child actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.actor.pid()
  }

  /// Returns the underlying actor reference.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor
  }

  /// Consumes the child reference and returns the underlying actor reference.
  #[must_use]
  pub fn into_actor_ref(self) -> ActorRef {
    self.actor
  }

  /// Sends a user message to the child actor.
  #[cfg(not(fraktor_disable_tell))]
  pub fn tell(&mut self, message: AnyMessage) {
    self.actor.tell(message);
  }

  /// Sends a user message to the child actor and preserves synchronous
  /// enqueue failures.
  ///
  /// # Errors
  ///
  /// Returns an error when the child mailbox rejects the message.
  pub fn try_tell(&mut self, message: AnyMessage) -> Result<(), SendError> {
    self.actor.try_tell(message)
  }

  /// Sends a request to the child actor and returns the associated ask response.
  ///
  /// Delivery failures and timeouts are observed through the returned ask
  /// response future.
  #[must_use]
  pub fn ask(&mut self, message: AnyMessage) -> AskResponse {
    self.actor.ask(message)
  }

  /// Sends a request to the child actor and arranges timeout completion on the response future.
  ///
  /// Delivery failures and timeouts are observed through the returned ask
  /// response future.
  #[must_use]
  pub fn ask_with_timeout(&mut self, message: AnyMessage, timeout: Duration) -> AskResponse {
    self.actor.ask_with_timeout(message, timeout)
  }

  /// Sends a stop signal to the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop signal cannot be delivered.
  pub fn stop(&self) -> Result<(), SendError> {
    self.system.send_system_message(self.pid(), SystemMessage::Stop)
  }

  /// Suspends the child mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the suspend signal cannot be delivered.
  pub fn suspend(&self) -> Result<(), SendError> {
    self.system.send_system_message(self.pid(), SystemMessage::Suspend)
  }

  /// Resumes the child mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the resume signal cannot be delivered.
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
