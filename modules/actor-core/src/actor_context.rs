//! Actor execution context utilities.

use core::marker::PhantomData;

use crate::{actor_ref::ActorRef, pid::Pid};

/// Provides contextual APIs while handling a message.
pub struct ActorContext<'a> {
  pid:      Pid,
  reply_to: Option<ActorRef>,
  _marker:  PhantomData<&'a ()>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context placeholder.
  #[must_use]
  pub const fn new(pid: Pid) -> Self {
    Self { pid, reply_to: None, _marker: PhantomData }
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
}
