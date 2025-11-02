//! Actor execution context utilities.

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_ref::ActorRef, any_message::AnyMessage, child_ref::ChildRef, pid::Pid,
  props::Props, send_error::SendError, spawn_error::SpawnError, system::ActorSystem,
};

/// Provides contextual APIs while handling a message.
pub struct ActorContext<'a, TB: RuntimeToolbox + 'static = NoStdToolbox> {
  system:   ActorSystem<TB>,
  pid:      Pid,
  reply_to: Option<ActorRef<TB>>,
  _marker:  PhantomData<&'a ()>,
}

impl<'a, TB: RuntimeToolbox + 'static> ActorContext<'a, TB> {
  /// Creates a new context placeholder.
  #[must_use]
  pub fn new(system: &ActorSystem<TB>, pid: Pid) -> Self {
    Self { system: system.clone(), pid, reply_to: None, _marker: PhantomData }
  }

  /// Returns a reference to the actor system.
  #[must_use]
  pub const fn system(&self) -> &ActorSystem<TB> {
    &self.system
  }

  /// Returns the pid of the running actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the reply target if supplied by the message envelope.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorRef<TB>> {
    self.reply_to.as_ref()
  }

  /// Sets the reply target (used internally by the runtime).
  pub fn set_reply_to(&mut self, reply_to: Option<ActorRef<TB>>) {
    self.reply_to = reply_to;
  }

  /// Clears the reply target after message processing completes.
  pub fn clear_reply_to(&mut self) {
    self.reply_to = None;
  }

  /// Returns an [`ActorRef`] pointing to the running actor.
  ///
  /// # Panics
  ///
  /// Panics if the actor reference cannot be resolved.
  #[must_use]
  pub fn self_ref(&self) -> ActorRef<TB> {
    match self.system.actor_ref(self.pid) {
      | Some(reference) => reference,
      | None => panic!("actor reference must exist for running context"),
    }
  }

  /// Sends a reply to the caller if a reply target is present.
  ///
  /// # Errors
  ///
  /// Returns an error if no reply target is set or sending fails.
  pub fn reply(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    match self.reply_to.as_ref() {
      | Some(target) => target.tell(message),
      | None => Err(SendError::no_recipient(message)),
    }
  }

  /// Requests the actor system to spawn a child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when spawning the child fails.
  pub fn spawn_child(&self, props: &Props<TB>) -> Result<ChildRef<TB>, SpawnError> {
    self.system.spawn_child(self.pid, props)
  }

  /// Returns the list of supervised children.
  #[must_use]
  pub fn children(&self) -> Vec<ChildRef<TB>> {
    self.system.children(self.pid)
  }

  /// Sends a stop signal to the specified child.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_child(&self, child: &ChildRef<TB>) -> Result<(), SendError<TB>> {
    child.stop()
  }

  /// Sends a stop signal to the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_self(&self) -> Result<(), SendError<TB>> {
    self.system.stop_actor(self.pid)
  }

  /// Suspends the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the suspend signal cannot be delivered.
  pub fn suspend_child(&self, child: &ChildRef<TB>) -> Result<(), SendError<TB>> {
    child.suspend()
  }

  /// Resumes the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the resume signal cannot be delivered.
  pub fn resume_child(&self, child: &ChildRef<TB>) -> Result<(), SendError<TB>> {
    child.resume()
  }
}
